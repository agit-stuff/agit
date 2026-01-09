//! Search command implementation.
//!
//! Provides subcommands for rebuilding the search index and querying it.

use std::collections::HashSet;
use std::path::Path;

use crate::cli::args::{SearchArgs, SearchCommands};
use crate::core::{detect_version, StorageVersion};
use crate::domain::{Category, IndexEntry, Role, WrappedBlob, WrappedNeuralCommit};
use crate::error::{AgitError, Result};
use crate::search::{indexer, retriever};
use crate::storage::{
    FileHeadStore, FileObjectStore, FileRefStore, GitObjectStore, GitRefStore, HeadStore,
    ObjectStore, RefStore,
};

use chrono::{TimeZone, Utc};
use git2::Repository;

/// Execute the search command.
pub fn execute(args: SearchArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    if !agit_dir.exists() {
        return Err(AgitError::NotInitialized);
    }

    match args.command {
        SearchCommands::Rebuild => rebuild_index(&cwd, &agit_dir),
        SearchCommands::Query(q) => query_index(&agit_dir, &q.query, q.limit),
    }
}

/// Rebuild the search index from existing neural commits.
fn rebuild_index(repo_path: &Path, agit_dir: &Path) -> Result<()> {
    println!("Rebuilding search index from neural commits...");

    // Detect storage version
    let is_v2 = match Repository::discover(repo_path) {
        Ok(repo) => matches!(detect_version(agit_dir, &repo), StorageVersion::V2GitNative),
        Err(_) => false,
    };

    // Delete existing index if present
    let index_path = agit_dir.join("search_index");
    if index_path.exists() {
        std::fs::remove_dir_all(&index_path)?;
        println!("  Removed existing index");
    }

    // Get all branches
    let head_store = FileHeadStore::new(agit_dir);
    let current_branch = head_store.get()?.unwrap_or_else(|| "main".to_string());

    let branches: Vec<String> = if is_v2 {
        let ref_store = GitRefStore::new(repo_path);
        ref_store.list()?
    } else {
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

    // Index all entries
    if all_entries.is_empty() {
        println!("No entries found to index.");
    } else {
        indexer::index_entries(agit_dir, &all_entries)?;
        println!(
            "Indexed {} entries from {} commits on {} branch(es)",
            all_entries.len(),
            visited_commits.len(),
            branches.len()
        );
    }

    // Hint about current branch
    if !branches.contains(&current_branch) && !branches.is_empty() {
        println!(
            "Note: Current branch '{}' has no neural commits yet.",
            current_branch
        );
    }

    Ok(())
}

/// Walk the commit chain and extract entries from trace blobs.
fn collect_entries_from_chain(
    repo_path: &Path,
    agit_dir: &Path,
    is_v2: bool,
    start_hash: &str,
    entries: &mut Vec<IndexEntry>,
    visited: &mut HashSet<String>,
) -> Result<()> {
    let mut current_hash = Some(start_hash.to_string());

    while let Some(hash) = current_hash {
        // Skip if already visited (handles merge commits)
        if visited.contains(&hash) {
            break;
        }
        visited.insert(hash.clone());

        // Load the neural commit
        let commit_data = if is_v2 {
            GitObjectStore::new(repo_path).load(&hash)?
        } else {
            FileObjectStore::new(agit_dir).load(&hash)?
        };

        let wrapped: WrappedNeuralCommit = serde_json::from_slice(&commit_data)?;
        let commit = &wrapped.data;

        // Load the trace blob
        let trace_result = if is_v2 {
            GitObjectStore::new(repo_path).load(&commit.trace_hash)
        } else {
            FileObjectStore::new(agit_dir).load(&commit.trace_hash)
        };

        if let Ok(trace_data) = trace_result {
            if let Ok(trace_blob) = serde_json::from_slice::<WrappedBlob>(&trace_data) {
                // Parse trace content into entries
                let parsed = parse_trace_content(&trace_blob.data.content, commit.created_at);
                entries.extend(parsed);
            }
        }

        // Move to parent
        current_hash = commit.first_parent().map(|s| s.to_string());
    }

    Ok(())
}

/// Parse trace content back into IndexEntry objects.
///
/// Trace format: `[HH:MM:SS] role/category: content`
fn parse_trace_content(
    trace: &str,
    commit_timestamp: chrono::DateTime<chrono::Utc>,
) -> Vec<IndexEntry> {
    let mut entries = Vec::new();

    for line in trace.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse format: [HH:MM:SS] role/category: content
        if let Some(rest) = line.strip_prefix('[') {
            if let Some(idx) = rest.find(']') {
                let after_bracket = &rest[idx + 1..].trim_start();

                // Parse role/category: content
                if let Some(colon_idx) = after_bracket.find(':') {
                    let role_cat = &after_bracket[..colon_idx];
                    let content = after_bracket[colon_idx + 1..].trim();

                    if let Some(slash_idx) = role_cat.find('/') {
                        let role_str = &role_cat[..slash_idx].trim();
                        let cat_str = &role_cat[slash_idx + 1..].trim();

                        let role = match *role_str {
                            "user" => Role::User,
                            "ai" => Role::Ai,
                            _ => continue,
                        };

                        let category = match *cat_str {
                            "intent" => Category::Intent,
                            "reasoning" => Category::Reasoning,
                            "error" => Category::Error,
                            "note" => Category::Note,
                            _ => continue,
                        };

                        entries.push(IndexEntry {
                            role,
                            category,
                            content: content.to_string(),
                            // Use commit timestamp since we don't have the exact time
                            timestamp: commit_timestamp,
                        });
                    }
                }
            }
        }
    }

    entries
}

/// Query the search index and display results.
fn query_index(agit_dir: &Path, query: &str, limit: usize) -> Result<()> {
    let results = retriever::search(agit_dir, query, limit)?;

    if results.is_empty() {
        println!("No results found for query: {}", query);
    } else {
        println!("Found {} result(s):\n", results.len());
        for (i, r) in results.iter().enumerate() {
            let timestamp = Utc.timestamp_opt(r.timestamp as i64, 0);
            let time_str = timestamp
                .single()
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".to_string());

            println!("{}. [{}] (score: {:.2})", i + 1, r.category, r.score);
            println!("   Time: {}", time_str);
            println!("   {}\n", r.body);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_parse_trace_content() {
        let trace = r#"[14:30:00] user/intent: Fix the auth bug
[14:30:15] ai/reasoning: Add try/catch around token validation
[14:30:30] ai/error: Failed to compile"#;

        let timestamp = Utc::now();
        let entries = parse_trace_content(trace, timestamp);

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].role, Role::User);
        assert_eq!(entries[0].category, Category::Intent);
        assert_eq!(entries[0].content, "Fix the auth bug");

        assert_eq!(entries[1].role, Role::Ai);
        assert_eq!(entries[1].category, Category::Reasoning);
        assert_eq!(entries[1].content, "Add try/catch around token validation");

        assert_eq!(entries[2].role, Role::Ai);
        assert_eq!(entries[2].category, Category::Error);
        assert_eq!(entries[2].content, "Failed to compile");
    }

    #[test]
    fn test_parse_trace_content_empty() {
        let trace = "";
        let timestamp = Utc::now();
        let entries = parse_trace_content(trace, timestamp);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_trace_content_invalid_lines() {
        let trace = r#"invalid line
[14:30:00] invalid format
[14:30:00] user/intent: Valid entry"#;

        let timestamp = Utc::now();
        let entries = parse_trace_content(trace, timestamp);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "Valid entry");
    }
}
