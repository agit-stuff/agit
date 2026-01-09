//! Implementation of the agit_log_step MCP tool.
//!
//! This tool allows AI editors to log steps (intents, reasoning) to the
//! AGIT index for later association with commits.

use std::path::Path;

use serde_json::Value;
use tracing::{debug, error};

use crate::domain::{Category, IndexEntry, Role};
use crate::mcp::protocol::{LogStepParams, ToolCallResult};
use crate::storage::{FileIndexStore, IndexStore};

/// Execute the agit_log_step tool.
pub fn execute(agit_dir: &Path, arguments: Option<Value>) -> ToolCallResult {
    // Parse arguments
    let args = match arguments {
        Some(v) => v,
        None => {
            return ToolCallResult::error("Missing arguments for agit_log_step");
        },
    };

    let params: LogStepParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => {
            error!("Invalid params for agit_log_step: {}", e);
            return ToolCallResult::error(&format!("Invalid parameters: {}", e));
        },
    };

    // Validate and convert role
    let role = match params.role.to_lowercase().as_str() {
        "user" => Role::User,
        "ai" => Role::Ai,
        _ => {
            return ToolCallResult::error(&format!(
                "Invalid role '{}'. Must be 'user' or 'ai'",
                params.role
            ));
        },
    };

    // Validate and convert category
    let category = match params.category.to_lowercase().as_str() {
        "intent" => Category::Intent,
        "reasoning" => Category::Reasoning,
        "error" => Category::Error,
        _ => {
            return ToolCallResult::error(&format!(
                "Invalid category '{}'. Must be 'intent', 'reasoning', or 'error'",
                params.category
            ));
        },
    };

    // Check if agit is initialized
    if !agit_dir.exists() {
        return ToolCallResult::error("AGIT not initialized. Run 'agit init' first.");
    }

    // Create the index entry
    let entry = IndexEntry::new(role, category, &params.content);

    // Append to index
    let index_store = FileIndexStore::new(agit_dir);
    if let Err(e) = index_store.append(&entry) {
        error!("Failed to append to index: {}", e);
        return ToolCallResult::error(&format!("Failed to log step: {}", e));
    }

    debug!(
        "Logged step: {}/{} - {}",
        params.role, params.category, params.content
    );

    ToolCallResult::text(&format!(
        "Logged: [{}/{}] {}",
        params.role,
        params.category,
        truncate(&params.content, 50)
    ))
}

/// Truncate a string for display.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn setup_agit_dir() -> TempDir {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();
        std::fs::write(agit_dir.join("index"), "").unwrap();
        temp
    }

    #[test]
    fn test_log_step_user_intent() {
        let temp = setup_agit_dir();
        let agit_dir = temp.path().join(".agit");

        let args = json!({
            "role": "user",
            "category": "intent",
            "content": "Fix the authentication bug"
        });

        let result = execute(&agit_dir, Some(args));
        assert!(result.is_error.is_none());
    }

    #[test]
    fn test_log_step_ai_reasoning() {
        let temp = setup_agit_dir();
        let agit_dir = temp.path().join(".agit");

        let args = json!({
            "role": "ai",
            "category": "reasoning",
            "content": "I'll add a try/catch block"
        });

        let result = execute(&agit_dir, Some(args));
        assert!(result.is_error.is_none());
    }

    #[test]
    fn test_log_step_invalid_role() {
        let temp = setup_agit_dir();
        let agit_dir = temp.path().join(".agit");

        let args = json!({
            "role": "invalid",
            "category": "intent",
            "content": "Test"
        });

        let result = execute(&agit_dir, Some(args));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_log_step_not_initialized() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");

        let args = json!({
            "role": "user",
            "category": "intent",
            "content": "Test"
        });

        let result = execute(&agit_dir, Some(args));
        assert_eq!(result.is_error, Some(true));
    }
}
