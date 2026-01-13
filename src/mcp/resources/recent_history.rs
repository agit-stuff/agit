//! Implementation of the agit://history/recent resource.
//!
//! This resource provides recent commit summaries to AI editors
//! so they can understand what was done recently in the project.

use std::path::Path;

use git2::Repository;
use tracing::debug;

use crate::core::{detect_version, StorageVersion};
use crate::domain::WrappedNeuralCommit;
use crate::mcp::protocol::ResourceContent;
use crate::storage::{
    FileHeadStore, FileObjectStore, FileRefStore, GitObjectStore, GitRefStore, HeadStore,
    ObjectStore, RefStore,
};

/// The URI for this resource.
pub const URI: &str = "agit://history/recent";

/// The human-readable name.
pub const NAME: &str = "Recent History";

/// Description of what this resource provides.
pub const DESCRIPTION: &str =
    "Recent commit summaries showing what was done recently in the project. \
     Use this to understand recent changes before starting work.";

/// MIME type for the content.
pub const MIME_TYPE: &str = "text/plain";

/// Default number of recent summaries to return.
const DEFAULT_COUNT: usize = 5;

/// Read the recent history resource.
pub fn read(project_root: &Path, agit_dir: &Path) -> Result<ResourceContent, String> {
    // Check if agit is initialized
    if !agit_dir.exists() {
        return Err("AGIT not initialized. Run 'agit init' first.".to_string());
    }

    // Get recent summaries (reuse logic from get_recent_summaries tool)
    match get_recent_summaries(project_root, agit_dir, DEFAULT_COUNT) {
        Ok(summaries) => Ok(ResourceContent::text(URI, &summaries, Some(MIME_TYPE))),
        Err(e) => {
            debug!("Failed to get recent summaries: {}", e);
            // Return a helpful message instead of an error for empty history
            let content = "No commits yet.\n\n\
                Recent summaries will appear here after you make commits with AGIT.\n\
                Use 'agit commit' to create commits that capture your reasoning.";
            Ok(ResourceContent::text(URI, content, Some(MIME_TYPE)))
        },
    }
}

/// Get recent commit summaries from the neural commit history.
/// This is extracted from get_recent_summaries tool for reuse.
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
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_read_not_initialized() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let agit_dir = temp.path().join(".agit");

        let result = read(project_root, &agit_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not initialized"));
    }

    #[test]
    fn test_read_no_commits() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let agit_dir = temp.path().join(".agit");

        // Create basic structure without any commits
        fs::create_dir_all(agit_dir.join("objects")).unwrap();
        fs::create_dir_all(agit_dir.join("refs/heads")).unwrap();
        fs::write(agit_dir.join("HEAD"), "main").unwrap();
        fs::write(agit_dir.join("index"), "").unwrap();

        let result = read(project_root, &agit_dir);
        // Should return OK with helpful message, not an error
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.text.unwrap().contains("No commits yet"));
    }
}
