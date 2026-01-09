//! Implementation of the agit_read_roadmap MCP tool.
//!
//! This tool allows AI editors to read the project roadmap to understand
//! the high-level goals and direction of the project.

use std::path::Path;

use tracing::debug;

use crate::domain::{ObjectType, WrappedBlob, WrappedNeuralCommit};
use crate::mcp::protocol::ToolCallResult;
use crate::storage::{FileHeadStore, FileObjectStore, FileRefStore, HeadStore, ObjectStore, RefStore};

/// Execute the agit_read_roadmap tool.
pub fn execute(agit_dir: &Path) -> ToolCallResult {
    // Check if agit is initialized
    if !agit_dir.exists() {
        return ToolCallResult::error("AGIT not initialized. Run 'agit init' first.");
    }

    // Try to find the latest roadmap from the most recent commit
    let roadmap = match read_latest_roadmap(agit_dir) {
        Ok(content) => content,
        Err(e) => {
            debug!("No roadmap found: {}", e);
            // Return a helpful message instead of an error
            return ToolCallResult::text(
                "No roadmap set yet.\n\n\
                 The roadmap captures high-level project goals and direction.\n\n\
                 To set a roadmap, create an 'agit commit' with context that includes\n\
                 your project goals, or add a ROADMAP.md file to your project.",
            );
        }
    };

    ToolCallResult::text(&roadmap)
}

/// Read the latest roadmap from the most recent neural commit.
fn read_latest_roadmap(agit_dir: &Path) -> Result<String, String> {
    // Get current branch
    let head_store = FileHeadStore::new(agit_dir);
    let branch = head_store.get()
        .map_err(|e| format!("Failed to read HEAD: {}", e))?
        .unwrap_or_else(|| "main".to_string());

    // Get the latest commit hash
    let ref_store = FileRefStore::new(agit_dir);
    let commit_hash = ref_store.get(&branch)
        .map_err(|e| format!("Failed to read ref: {}", e))?
        .ok_or_else(|| "No commits yet".to_string())?;

    // Load the commit
    let object_store = FileObjectStore::new(agit_dir);
    let commit_data = object_store.load(&commit_hash)
        .map_err(|e| format!("Failed to load commit: {}", e))?;

    let wrapped: WrappedNeuralCommit = serde_json::from_slice(&commit_data)
        .map_err(|e| format!("Failed to parse commit: {}", e))?;

    // Load the roadmap blob
    let roadmap_data = object_store.load(&wrapped.data.roadmap_hash)
        .map_err(|e| format!("Failed to load roadmap: {}", e))?;

    let roadmap_blob: WrappedBlob = serde_json::from_slice(&roadmap_data)
        .map_err(|e| format!("Failed to parse roadmap: {}", e))?;

    if roadmap_blob.object_type != ObjectType::Blob {
        return Err("Invalid roadmap object type".to_string());
    }

    Ok(roadmap_blob.data.content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_read_roadmap_not_initialized() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");

        let result = execute(&agit_dir);
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_read_roadmap_no_commits() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");

        // Create basic structure without any commits
        std::fs::create_dir_all(agit_dir.join("objects")).unwrap();
        std::fs::create_dir_all(agit_dir.join("refs/heads")).unwrap();
        std::fs::write(agit_dir.join("HEAD"), "main").unwrap();
        std::fs::write(agit_dir.join("index"), "").unwrap();

        let result = execute(&agit_dir);
        // Should return a helpful message, not an error
        assert!(result.is_error.is_none());
    }
}
