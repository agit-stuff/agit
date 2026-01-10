//! Index state tracking for incremental search indexing.
//!
//! Tracks which neural commits have been indexed to enable efficient
//! incremental updates after `agit pull`.

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::error::Result;

const INDEXED_COMMITS_FILE: &str = ".indexed_commits";

/// Load the set of already-indexed commit hashes.
///
/// Returns an empty set if the file doesn't exist.
pub fn load_indexed_commits(agit_dir: &Path) -> Result<HashSet<String>> {
    let index_path = agit_dir.join("search_index");
    let state_file = index_path.join(INDEXED_COMMITS_FILE);

    if !state_file.exists() {
        return Ok(HashSet::new());
    }

    let file = fs::File::open(&state_file)?;
    let reader = BufReader::new(file);
    let mut commits = HashSet::new();

    for line in reader.lines() {
        let line = line?;
        let hash = line.trim();
        if !hash.is_empty() {
            commits.insert(hash.to_string());
        }
    }

    Ok(commits)
}

/// Save the set of indexed commit hashes.
pub fn save_indexed_commits(agit_dir: &Path, commits: &HashSet<String>) -> Result<()> {
    let index_path = agit_dir.join("search_index");
    fs::create_dir_all(&index_path)?;

    let state_file = index_path.join(INDEXED_COMMITS_FILE);
    let mut file = fs::File::create(&state_file)?;

    for hash in commits {
        writeln!(file, "{}", hash)?;
    }

    Ok(())
}

/// Mark a single commit as indexed by adding it to the state file.
pub fn mark_commit_indexed(agit_dir: &Path, hash: &str) -> Result<()> {
    let mut commits = load_indexed_commits(agit_dir)?;
    commits.insert(hash.to_string());
    save_indexed_commits(agit_dir, &commits)
}

/// Check if the search index exists and is in a healthy state.
///
/// Returns true if the index directory exists and contains the necessary files.
pub fn is_index_healthy(agit_dir: &Path) -> bool {
    let index_path = agit_dir.join("search_index");

    if !index_path.exists() {
        return false;
    }

    // Check for meta.json which indicates a valid Tantivy index
    let meta_file = index_path.join("meta.json");
    meta_file.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_empty() {
        let temp_dir = TempDir::new().unwrap();
        let agit_dir = temp_dir.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();

        let commits = load_indexed_commits(&agit_dir).unwrap();
        assert!(commits.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let agit_dir = temp_dir.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();

        let mut commits = HashSet::new();
        commits.insert("abc123".to_string());
        commits.insert("def456".to_string());

        save_indexed_commits(&agit_dir, &commits).unwrap();
        let loaded = load_indexed_commits(&agit_dir).unwrap();

        assert_eq!(commits, loaded);
    }

    #[test]
    fn test_mark_commit_indexed() {
        let temp_dir = TempDir::new().unwrap();
        let agit_dir = temp_dir.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();

        mark_commit_indexed(&agit_dir, "abc123").unwrap();
        mark_commit_indexed(&agit_dir, "def456").unwrap();

        let commits = load_indexed_commits(&agit_dir).unwrap();
        assert!(commits.contains("abc123"));
        assert!(commits.contains("def456"));
    }

    #[test]
    fn test_is_index_healthy() {
        let temp_dir = TempDir::new().unwrap();
        let agit_dir = temp_dir.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();

        // No index - not healthy
        assert!(!is_index_healthy(&agit_dir));

        // Create index dir but no meta.json - not healthy
        let index_path = agit_dir.join("search_index");
        fs::create_dir_all(&index_path).unwrap();
        assert!(!is_index_healthy(&agit_dir));

        // Create meta.json - healthy
        fs::write(index_path.join("meta.json"), "{}").unwrap();
        assert!(is_index_healthy(&agit_dir));
    }
}
