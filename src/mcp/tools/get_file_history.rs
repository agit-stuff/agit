//! Get file history tool - retrieves neural commit history for a specific file.
//!
//! Uses an optimized approach: pre-calculate interesting git hashes using
//! `git log --format=%H -- <filepath>` for O(1) lookup instead of expensive
//! tree-to-tree diffs.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use git2::Repository;
use serde::Deserialize;
use serde_json::Value;
use tracing::debug;

use crate::core::{detect_version, StorageVersion};
use crate::domain::WrappedNeuralCommit;
use crate::mcp::protocol::ToolCallResult;
use crate::storage::{
    FileHeadStore, FileObjectStore, FileRefStore, GitObjectStore, GitRefStore, HeadStore,
    ObjectStore, RefStore,
};

#[derive(Debug, Deserialize)]
struct GetFileHistoryParams {
    filepath: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    3
}

/// Execute the get_file_history tool.
pub fn execute(project_root: &Path, agit_dir: &Path, arguments: Option<Value>) -> ToolCallResult {
    let args = match arguments {
        Some(v) => v,
        None => {
            return ToolCallResult::error("Missing arguments: 'filepath' is required");
        },
    };

    let params: GetFileHistoryParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => {
            return ToolCallResult::error(&format!("Invalid parameters: {}", e));
        },
    };

    if !agit_dir.exists() {
        return ToolCallResult::error("AGIT not initialized. Run 'agit init' first.");
    }

    match find_file_history(project_root, agit_dir, &params.filepath, params.limit) {
        Ok(history) => {
            if history.is_empty() {
                ToolCallResult::text(&format!("No history found for file: {}", params.filepath))
            } else {
                ToolCallResult::text(&history)
            }
        },
        Err(e) => {
            debug!("Failed to get file history: {}", e);
            ToolCallResult::error(&format!("Error: {}", e))
        },
    }
}

/// Find neural commits that touched a specific file.
///
/// Uses an optimized approach:
/// 1. Run `git log --format=%H -- <filepath>` to get all commits that touched the file
/// 2. Store these hashes in a HashSet for O(1) lookup
/// 3. Walk the Agit neural commit chain and match against the HashSet
fn find_file_history(
    project_root: &Path,
    agit_dir: &Path,
    filepath: &str,
    limit: usize,
) -> Result<String, String> {
    // 1. OPTIMIZATION: Ask Git for the relevant hashes first
    // This replaces the expensive tree-to-tree diff approach
    let output = Command::new("git")
        .current_dir(project_root)
        .args(["log", "--format=%H", "--", filepath])
        .output()
        .map_err(|e| format!("Failed to execute git log: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // If the file doesn't exist in git history, that's OK - just return empty
        if stderr.contains("does not have any commits yet") || stderr.contains("unknown revision") {
            return Ok(String::new());
        }
        return Err(stderr.to_string());
    }

    let raw_output = String::from_utf8_lossy(&output.stdout);
    let interesting_hashes: HashSet<&str> = raw_output.lines().map(|l| l.trim()).collect();

    if interesting_hashes.is_empty() {
        return Ok(String::new()); // Git says this file was never touched
    }

    debug!(
        "Found {} git commits that touched '{}'",
        interesting_hashes.len(),
        filepath
    );

    // 2. Detect storage version and setup stores
    let is_v2 = match Repository::discover(project_root) {
        Ok(repo) => matches!(detect_version(agit_dir, &repo), StorageVersion::V2GitNative),
        Err(_) => false,
    };

    // Get current branch
    let head_store = FileHeadStore::new(agit_dir);
    let branch = head_store
        .get()
        .map_err(|e| format!("Failed to get branch: {}", e))?
        .unwrap_or_else(|| "main".to_string());

    // Get the starting neural commit hash
    let start_hash = if is_v2 {
        let ref_store = GitRefStore::new(project_root);
        ref_store
            .get(&branch)
            .map_err(|e| format!("Failed to get ref: {}", e))?
    } else {
        let ref_store = FileRefStore::new(agit_dir);
        ref_store
            .get(&branch)
            .map_err(|e| format!("Failed to get ref: {}", e))?
    };

    let start_hash = match start_hash {
        Some(h) => h,
        None => return Ok(String::new()),
    };

    // 3. Walk the Agit neural commit chain and find commits that touched this file
    let mut results: Vec<(String, String)> = Vec::new(); // (git_hash, summary)
    let mut current_hash = Some(start_hash);
    let mut scanned_count = 0;
    let max_scan_depth = 500; // Safety brake to prevent runaway scans

    while let Some(hash) = current_hash {
        if results.len() >= limit || scanned_count > max_scan_depth {
            break;
        }
        scanned_count += 1;

        // Load the neural commit
        let commit_data = if is_v2 {
            GitObjectStore::new(project_root)
                .load(&hash)
                .map_err(|e| format!("Failed to load commit: {}", e))?
        } else {
            FileObjectStore::new(agit_dir)
                .load(&hash)
                .map_err(|e| format!("Failed to load commit: {}", e))?
        };

        let wrapped: WrappedNeuralCommit = serde_json::from_slice(&commit_data)
            .map_err(|e| format!("Failed to parse commit: {}", e))?;
        let commit = wrapped.data;

        // 4. The Fast Check - O(1) HashSet lookup
        if interesting_hashes.contains(commit.git_hash.as_str()) {
            results.push((commit.git_hash.clone(), commit.summary.clone()));
        }

        // Move to parent
        current_hash = commit.first_parent().map(|s| s.to_string());
    }

    // 5. Format results
    if results.is_empty() {
        return Ok(String::new());
    }

    let formatted: Vec<String> = results
        .iter()
        .enumerate()
        .map(|(i, (git_hash, summary))| {
            let short_hash = &git_hash[..7.min(git_hash.len())];
            format!("{}. [{}] {}", i + 1, short_hash, summary)
        })
        .collect();

    Ok(format!(
        "File history for '{}':\n\n{}",
        filepath,
        formatted.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_get_file_history_not_initialized() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let agit_dir = temp.path().join(".agit");

        let args = json!({
            "filepath": "src/main.rs"
        });

        let result = execute(project_root, &agit_dir, Some(args));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_get_file_history_missing_args() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let agit_dir = temp.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        let result = execute(project_root, &agit_dir, None);
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_get_file_history_invalid_params() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let agit_dir = temp.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        let args = json!({
            "wrong_field": "value"
        });

        let result = execute(project_root, &agit_dir, Some(args));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 3);
    }
}
