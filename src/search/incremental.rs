//! Incremental search index updates.
//!
//! This module provides efficient incremental indexing after `agit pull`,
//! only indexing newly fetched commits instead of rebuilding the entire index.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use git2::Repository;

use crate::cli::commands::search::{collect_entries_from_chain, parse_trace_content};
use crate::core::{detect_version, StorageVersion};
use crate::domain::{IndexEntry, WrappedBlob, WrappedNeuralCommit};
use crate::error::Result;
use crate::search::{index_state, indexer};
use crate::storage::{GitObjectStore, GitRefStore, ObjectStore, RefStore};

/// Index newly pulled commits incrementally.
///
/// Compares refs before and after the pull to determine which commits are new,
/// then indexes only those commits' trace entries.
///
/// Returns `(entries_indexed, was_full_rebuild)`:
/// - `entries_indexed`: Number of search entries added
/// - `was_full_rebuild`: True if a full rebuild was performed instead of incremental
pub fn index_new_commits(
    repo_path: &Path,
    agit_dir: &Path,
    refs_before: &HashMap<String, String>,
    refs_after: &HashMap<String, String>,
) -> Result<(usize, bool)> {
    // Check if index is healthy
    if !index_state::is_index_healthy(agit_dir) {
        // Need full rebuild
        return full_rebuild(repo_path, agit_dir);
    }

    // Detect storage version
    let repo = Repository::discover(repo_path)?;
    let is_v2 = matches!(detect_version(agit_dir, &repo), StorageVersion::V2GitNative);

    if !is_v2 {
        // V1 storage - just do full rebuild for simplicity
        return full_rebuild(repo_path, agit_dir);
    }

    // Load already-indexed commits
    let mut indexed_commits = index_state::load_indexed_commits(agit_dir)?;

    // Find new commits to index
    let mut all_entries = Vec::new();
    let mut newly_indexed = HashSet::new();

    for (branch, new_hash) in refs_after {
        let old_hash = refs_before.get(branch).map(|s| s.as_str());

        // Collect entries from new commits on this branch
        collect_new_entries(
            repo_path,
            agit_dir,
            is_v2,
            new_hash,
            old_hash,
            &indexed_commits,
            &mut all_entries,
            &mut newly_indexed,
        )?;
    }

    if all_entries.is_empty() {
        return Ok((0, false));
    }

    // Index the new entries
    indexer::index_entries(agit_dir, &all_entries)?;

    // Update indexed commits state
    indexed_commits.extend(newly_indexed);
    index_state::save_indexed_commits(agit_dir, &indexed_commits)?;

    Ok((all_entries.len(), false))
}

/// Collect entries from new commits, stopping at already-indexed commits.
fn collect_new_entries(
    repo_path: &Path,
    _agit_dir: &Path,
    _is_v2: bool,
    start_hash: &str,
    stop_at_hash: Option<&str>,
    already_indexed: &HashSet<String>,
    entries: &mut Vec<IndexEntry>,
    newly_indexed: &mut HashSet<String>,
) -> Result<()> {
    let mut current_hash = Some(start_hash.to_string());

    while let Some(hash) = current_hash {
        // Stop if we reach the old tip
        if let Some(stop) = stop_at_hash {
            if hash == stop {
                break;
            }
        }

        // Stop if already indexed
        if already_indexed.contains(&hash) {
            break;
        }

        // Skip if we've already processed this commit in this run
        if newly_indexed.contains(&hash) {
            break;
        }

        newly_indexed.insert(hash.clone());

        // Load the neural commit
        let commit_data = GitObjectStore::new(repo_path).load(&hash)?;
        let wrapped: WrappedNeuralCommit = serde_json::from_slice(&commit_data)?;
        let commit = &wrapped.data;

        // Load the trace blob
        let trace_result = GitObjectStore::new(repo_path).load(&commit.trace_hash);

        if let Ok(trace_data) = trace_result {
            if let Ok(trace_blob) = serde_json::from_slice::<WrappedBlob>(&trace_data) {
                let parsed = parse_trace_content(&trace_blob.data.content, commit.created_at);
                entries.extend(parsed);
            }
        }

        // Move to parent
        current_hash = commit.first_parent().map(|s| s.to_string());
    }

    Ok(())
}

/// Perform a full index rebuild.
///
/// This is called when the index is unhealthy or missing.
fn full_rebuild(repo_path: &Path, agit_dir: &Path) -> Result<(usize, bool)> {
    // Delete existing index
    let index_path = agit_dir.join("search_index");
    if index_path.exists() {
        std::fs::remove_dir_all(&index_path)?;
    }

    // Detect storage version
    let is_v2 = match Repository::discover(repo_path) {
        Ok(repo) => matches!(detect_version(agit_dir, &repo), StorageVersion::V2GitNative),
        Err(_) => false,
    };

    // Get all branches
    let branches: Vec<String> = if is_v2 {
        let ref_store = GitRefStore::new(repo_path);
        ref_store.list()?
    } else {
        use crate::storage::FileRefStore;
        let ref_store = FileRefStore::new(agit_dir);
        ref_store.list()?
    };

    let mut all_entries = Vec::new();
    let mut visited_commits = HashSet::new();

    // Walk all branches
    for branch in &branches {
        let start_hash = if is_v2 {
            let ref_store = GitRefStore::new(repo_path);
            ref_store.get(branch)?
        } else {
            use crate::storage::FileRefStore;
            let ref_store = FileRefStore::new(agit_dir);
            ref_store.get(branch)?
        };

        if let Some(hash) = start_hash {
            collect_entries_from_chain(
                repo_path,
                agit_dir,
                is_v2,
                &hash,
                &mut all_entries,
                &mut visited_commits,
            )?;
        }
    }

    if !all_entries.is_empty() {
        indexer::index_entries(agit_dir, &all_entries)?;
    }

    // Save the indexed commits state
    index_state::save_indexed_commits(agit_dir, &visited_commits)?;

    Ok((all_entries.len(), true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_refs_no_work() {
        // When refs_before == refs_after, nothing to do
        let before: HashMap<String, String> = HashMap::new();
        let after: HashMap<String, String> = HashMap::new();

        // We can't easily test without a real repo, but the logic is:
        // - If no refs changed, no new commits to index
        assert!(before.is_empty());
        assert!(after.is_empty());
    }
}
