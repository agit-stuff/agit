//! Implementation of the agit_get_recent_summaries MCP tool.
//!
//! This tool allows AI editors to see recent commit summaries to understand
//! what was done recently in the project.

use std::path::Path;

use git2::Repository;
use serde_json::Value;
use tracing::debug;

use crate::core::{detect_version, StorageVersion};
use crate::domain::WrappedNeuralCommit;
use crate::mcp::protocol::{GetRecentSummariesParams, ToolCallResult};
use crate::storage::{
    FileHeadStore, FileObjectStore, FileRefStore, GitObjectStore, GitRefStore, HeadStore,
    ObjectStore, RefStore,
};

/// Default number of recent summaries to return.
const DEFAULT_COUNT: usize = 5;

/// Execute the agit_get_recent_summaries tool.
pub fn execute(project_root: &Path, agit_dir: &Path, arguments: Option<Value>) -> ToolCallResult {
    // Parse arguments (count is optional)
    let count = match arguments {
        Some(v) => {
            match serde_json::from_value::<GetRecentSummariesParams>(v) {
                Ok(p) => p.count.unwrap_or(DEFAULT_COUNT),
                Err(_) => DEFAULT_COUNT,
            }
        },
        None => DEFAULT_COUNT,
    };

    // Check if agit is initialized
    if !agit_dir.exists() {
        return ToolCallResult::error("AGIT not initialized. Run 'agit init' first.");
    }

    // Get recent summaries
    match get_recent_summaries(project_root, agit_dir, count) {
        Ok(summaries) => ToolCallResult::text(&summaries),
        Err(e) => {
            debug!("Failed to get recent summaries: {}", e);
            // Return a helpful message instead of an error for empty history
            ToolCallResult::text(
                "No commits yet.\n\n\
                 Recent summaries will appear here after you make commits with AGIT.\n\
                 Use 'agit commit' to create commits that capture your reasoning.",
            )
        },
    }
}

/// Get recent commit summaries from the neural commit history.
fn get_recent_summaries(
    project_root: &Path,
    agit_dir: &Path,
    count: usize,
) -> Result<String, String> {
    let head_store = FileHeadStore::new(agit_dir);

    // Detect storage version
    let is_v2 = match Repository::discover(project_root) {
        Ok(repo) => matches!(detect_version(agit_dir, &repo), StorageVersion::V2GitNative),
        Err(_) => false,
    };

    // Get current branch
    let branch = head_store
        .get()
        .map_err(|e| format!("Failed to read HEAD: {}", e))?
        .unwrap_or_else(|| "main".to_string());

    // Start from the latest commit and walk back
    let mut current_hash: Option<String> = if is_v2 {
        let ref_store = GitRefStore::new(project_root);
        ref_store
            .get(&branch)
            .map_err(|e| format!("Failed to read ref: {}", e))?
    } else {
        let ref_store = FileRefStore::new(agit_dir);
        ref_store
            .get(&branch)
            .map_err(|e| format!("Failed to read ref: {}", e))?
    };

    if current_hash.is_none() {
        return Err("No commits yet".to_string());
    }

    let mut summaries = Vec::new();

    while let Some(hash) = current_hash {
        if summaries.len() >= count {
            break;
        }

        // Load the commit
        let commit_data = if is_v2 {
            let object_store = GitObjectStore::new(project_root);
            object_store
                .load(&hash)
                .map_err(|e| format!("Failed to load commit: {}", e))?
        } else {
            let object_store = FileObjectStore::new(agit_dir);
            object_store
                .load(&hash)
                .map_err(|e| format!("Failed to load commit: {}", e))?
        };

        let wrapped: WrappedNeuralCommit = serde_json::from_slice(&commit_data)
            .map_err(|e| format!("Failed to parse commit: {}", e))?;

        let commit = &wrapped.data;

        // Format: ## [short_hash] date\nsummary
        let date_str = commit.created_at.format("%Y-%m-%d").to_string();
        summaries.push(format!(
            "## [{}] {}\n{}",
            commit.short_hash(),
            date_str,
            commit.summary
        ));

        current_hash = commit.first_parent().map(|s| s.to_string());
    }

    if summaries.is_empty() {
        return Err("No commits found".to_string());
    }

    let mut output = String::from("# Recent Activity\n\n");
    output.push_str(&summaries.join("\n\n"));

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_get_recent_summaries_not_initialized() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let agit_dir = temp.path().join(".agit");

        let result = execute(project_root, &agit_dir, None);
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_get_recent_summaries_no_commits() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let agit_dir = temp.path().join(".agit");

        // Create basic structure without any commits
        std::fs::create_dir_all(agit_dir.join("objects")).unwrap();
        std::fs::create_dir_all(agit_dir.join("refs/heads")).unwrap();
        std::fs::write(agit_dir.join("HEAD"), "main").unwrap();
        std::fs::write(agit_dir.join("index"), "").unwrap();

        let result = execute(project_root, &agit_dir, None);
        // Should return a helpful message, not an error
        assert!(result.is_error.is_none());
    }

    #[test]
    fn test_get_recent_summaries_with_count() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let agit_dir = temp.path().join(".agit");

        // Create basic structure
        std::fs::create_dir_all(agit_dir.join("objects")).unwrap();
        std::fs::create_dir_all(agit_dir.join("refs/heads")).unwrap();
        std::fs::write(agit_dir.join("HEAD"), "main").unwrap();

        let args = json!({
            "count": 3
        });

        let result = execute(project_root, &agit_dir, Some(args));
        // Should return a helpful message for empty history
        assert!(result.is_error.is_none());
    }
}
