//! Implementation of the agit_get_context MCP tool.
//!
//! This tool allows AI editors to retrieve the context (intent, reasoning)
//! associated with a specific git commit.

use std::path::Path;

use serde_json::Value;
use tracing::{debug, error};

use crate::domain::{ObjectType, WrappedBlob, WrappedNeuralCommit};
use crate::mcp::protocol::{GetContextParams, ToolCallResult};
use crate::storage::{FileHeadStore, FileObjectStore, FileRefStore, HeadStore, ObjectStore, RefStore};

/// Execute the agit_get_context tool.
pub fn execute(agit_dir: &Path, arguments: Option<Value>) -> ToolCallResult {
    // Parse arguments
    let args = match arguments {
        Some(v) => v,
        None => {
            return ToolCallResult::error("Missing arguments for agit_get_context");
        }
    };

    let params: GetContextParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => {
            error!("Invalid params for agit_get_context: {}", e);
            return ToolCallResult::error(&format!("Invalid parameters: {}", e));
        }
    };

    // Check if agit is initialized
    if !agit_dir.exists() {
        return ToolCallResult::error("AGIT not initialized. Run 'agit init' first.");
    }

    // Find and return the context
    match find_context_for_git_hash(agit_dir, &params.git_hash) {
        Ok(context) => ToolCallResult::text(&context),
        Err(e) => {
            debug!("Failed to find context: {}", e);
            ToolCallResult::error(&format!("No context found for git hash '{}': {}", params.git_hash, e))
        }
    }
}

/// Find the neural commit context for a given git hash.
fn find_context_for_git_hash(agit_dir: &Path, git_hash: &str) -> Result<String, String> {
    let object_store = FileObjectStore::new(agit_dir);
    let ref_store = FileRefStore::new(agit_dir);
    let head_store = FileHeadStore::new(agit_dir);

    // Get current branch
    let branch = head_store.get()
        .map_err(|e| format!("Failed to read HEAD: {}", e))?
        .unwrap_or_else(|| "main".to_string());

    // Start from the latest commit and walk back
    let mut current_hash = ref_store.get(&branch)
        .map_err(|e| format!("Failed to read ref: {}", e))?;

    while let Some(hash) = current_hash {
        // Load the commit
        let commit_data = object_store.load(&hash)
            .map_err(|e| format!("Failed to load commit: {}", e))?;

        let wrapped: WrappedNeuralCommit = serde_json::from_slice(&commit_data)
            .map_err(|e| format!("Failed to parse commit: {}", e))?;

        let commit = &wrapped.data;

        // Check if git hash matches (prefix match)
        if commit.git_hash.starts_with(git_hash) || git_hash.starts_with(&commit.git_hash) {
            // Found it! Build the context response
            let mut context = String::new();

            context.push_str(&format!("# Context for git commit {}\n\n", commit.git_hash));
            context.push_str(&format!("**Neural Commit:** {}\n", hash));
            context.push_str(&format!("**Author:** {}\n", commit.author));
            context.push_str(&format!("**Date:** {}\n\n", commit.created_at.format("%Y-%m-%d %H:%M:%S UTC")));
            context.push_str(&format!("## Summary\n\n{}\n\n", commit.summary));

            // Try to load the trace
            if let Ok(trace_data) = object_store.load(&commit.trace_hash) {
                if let Ok(trace_blob) = serde_json::from_slice::<WrappedBlob>(&trace_data) {
                    if trace_blob.object_type == ObjectType::Blob {
                        context.push_str("## Trace\n\n```\n");
                        context.push_str(&trace_blob.data.content);
                        context.push_str("\n```\n");
                    }
                }
            }

            return Ok(context);
        }

        current_hash = commit.parent_hash.clone();
    }

    Err("Not found in neural commit history".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_get_context_not_initialized() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");

        let args = json!({
            "git_hash": "abc123"
        });

        let result = execute(&agit_dir, Some(args));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_get_context_missing_args() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        let result = execute(&agit_dir, None);
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_get_context_no_commits() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");

        // Create basic structure
        std::fs::create_dir_all(agit_dir.join("objects")).unwrap();
        std::fs::create_dir_all(agit_dir.join("refs/heads")).unwrap();
        std::fs::write(agit_dir.join("HEAD"), "main").unwrap();

        let args = json!({
            "git_hash": "abc123"
        });

        let result = execute(&agit_dir, Some(args));
        assert_eq!(result.is_error, Some(true));
    }
}
