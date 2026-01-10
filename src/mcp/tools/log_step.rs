//! Implementation of the agit_log_step MCP tool.
//!
//! This tool allows AI editors to log steps (intents, reasoning) to the
//! AGIT index for later association with commits.
//!
//! Supports both single-entry mode (backward compatible) and batch mode
//! for efficiency in environments with tool authorization popups.

use std::path::Path;

use serde_json::Value;
use tracing::{debug, error, warn};

use crate::core::ensure_sync;
use crate::domain::{Category, IndexEntry, Role};
#[cfg(test)]
use crate::mcp::protocol::ToolContent;
use crate::mcp::protocol::{LogEntry, LogStepParams, ToolCallResult};
use crate::safety::validate_path_is_internal;
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

    // Check if agit is initialized
    if !agit_dir.exists() {
        return ToolCallResult::error("AGIT not initialized. Run 'agit init' first.");
    }

    // Ensure branch sync (derive project root from agit_dir)
    if let Some(project_root) = agit_dir.parent() {
        if let Err(e) = ensure_sync(project_root, agit_dir) {
            warn!("Branch sync failed: {}", e);
            // Continue anyway - sync failure shouldn't block logging
        }
    }

    // Determine if batch or single mode
    if let Some(batch) = params.batch {
        // Batch mode - process multiple entries
        execute_batch(agit_dir, batch)
    } else if let (Some(role), Some(category), Some(content)) =
        (params.role, params.category, params.content)
    {
        // Single mode (backward compatible)
        execute_single(
            agit_dir,
            &role,
            &category,
            &content,
            params.file_path.as_deref(),
            params.line_number,
        )
    } else {
        ToolCallResult::error(
            "Invalid parameters: provide either 'batch' array or 'role', 'category', 'content'",
        )
    }
}

/// Execute batch logging - multiple entries in one call.
fn execute_batch(agit_dir: &Path, entries: Vec<LogEntry>) -> ToolCallResult {
    if entries.is_empty() {
        return ToolCallResult::text("No entries to log");
    }

    // Derive repo root from agit_dir
    let repo_root = match agit_dir.parent() {
        Some(root) => root,
        None => return ToolCallResult::error("Cannot determine repository root"),
    };

    let index_store = FileIndexStore::new(agit_dir);
    let mut logged = 0;
    let mut errors = Vec::new();
    let mut rejected_paths = Vec::new();

    for entry in &entries {
        // Validate file_path is within repository boundary
        if let Some(ref file_path) = entry.file_path {
            if let Err(e) = validate_path_is_internal(repo_root, file_path) {
                rejected_paths.push(format!("{}: {}", file_path, e));
                continue; // Skip this entry
            }
        }

        // Validate role
        let role = match entry.role.to_lowercase().as_str() {
            "user" => Role::User,
            "ai" => Role::Ai,
            _ => {
                errors.push(format!("Invalid role '{}'", entry.role));
                continue;
            },
        };

        // Validate category
        let category = match entry.category.to_lowercase().as_str() {
            "intent" => Category::Intent,
            "reasoning" => Category::Reasoning,
            "error" => Category::Error,
            _ => {
                errors.push(format!("Invalid category '{}'", entry.category));
                continue;
            },
        };

        // Create and append entry with optional file/line location
        let index_entry = IndexEntry::with_location(
            role,
            category,
            &entry.content,
            entry.file_path.clone(),
            entry.line_number,
        );
        if let Err(e) = index_store.append(&index_entry) {
            errors.push(format!("Failed to log: {}", e));
            continue;
        }

        logged += 1;
        debug!(
            "Logged: {}/{} - {}",
            entry.role,
            entry.category,
            truncate(&entry.content, 50)
        );
    }

    // Build response
    if !rejected_paths.is_empty() {
        let rejection_msg = format!(
            "⛔ {} entries rejected (outside repository scope):\n{}\n\nAgit is a single-repo tool. Use `cd` to switch to the correct repository before logging context for those files.",
            rejected_paths.len(),
            rejected_paths.join("\n")
        );

        if logged == 0 && errors.is_empty() {
            return ToolCallResult::error(&rejection_msg);
        } else {
            // Some entries logged, but some rejected
            let mut msg = format!("Logged {} entries.", logged);
            if !errors.is_empty() {
                msg.push_str(&format!(" {} errors: {}", errors.len(), errors.join("; ")));
            }
            msg.push_str(&format!("\n{}", rejection_msg));
            return ToolCallResult::text(&msg);
        }
    }

    if errors.is_empty() {
        ToolCallResult::text(&format!("Logged {} entries", logged))
    } else if logged > 0 {
        ToolCallResult::text(&format!(
            "Logged {} entries with {} errors: {}",
            logged,
            errors.len(),
            errors.join("; ")
        ))
    } else {
        ToolCallResult::error(&format!("All entries failed: {}", errors.join("; ")))
    }
}

/// Execute single entry logging (backward compatible).
fn execute_single(
    agit_dir: &Path,
    role: &str,
    category: &str,
    content: &str,
    file_path: Option<&str>,
    line_number: Option<u32>,
) -> ToolCallResult {
    // Derive repo root from agit_dir
    let repo_root = match agit_dir.parent() {
        Some(root) => root,
        None => return ToolCallResult::error("Cannot determine repository root"),
    };

    // Validate file_path is within repository boundary
    if let Some(fp) = file_path {
        if let Err(e) = validate_path_is_internal(repo_root, fp) {
            return ToolCallResult::error(&format!(
                "⛔ Path rejected: {}\n\nAgit is a single-repo tool. Use `cd` to switch to the correct repository before logging context for external files.",
                e
            ));
        }
    }

    // Validate role
    let role_enum = match role.to_lowercase().as_str() {
        "user" => Role::User,
        "ai" => Role::Ai,
        _ => {
            return ToolCallResult::error(&format!(
                "Invalid role '{}'. Must be 'user' or 'ai'",
                role
            ));
        },
    };

    // Validate category
    let category_enum = match category.to_lowercase().as_str() {
        "intent" => Category::Intent,
        "reasoning" => Category::Reasoning,
        "error" => Category::Error,
        _ => {
            return ToolCallResult::error(&format!(
                "Invalid category '{}'. Must be 'intent', 'reasoning', or 'error'",
                category
            ));
        },
    };

    // Create and append entry with optional file/line location
    let entry = IndexEntry::with_location(
        role_enum,
        category_enum,
        content,
        file_path.map(|s| s.to_string()),
        line_number,
    );
    let index_store = FileIndexStore::new(agit_dir);

    if let Err(e) = index_store.append(&entry) {
        error!("Failed to append to index: {}", e);
        return ToolCallResult::error(&format!("Failed to log step: {}", e));
    }

    debug!("Logged step: {}/{} - {}", role, category, content);
    ToolCallResult::text(&format!(
        "Logged: [{}/{}] {}",
        role,
        category,
        truncate(content, 50)
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

    /// Extract text from ToolContent for assertions.
    fn get_text(content: &ToolContent) -> &str {
        match content {
            ToolContent::Text { text } => text,
        }
    }

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

    #[test]
    fn test_log_step_batch_mode() {
        let temp = setup_agit_dir();
        let agit_dir = temp.path().join(".agit");

        let args = json!({
            "batch": [
                {"role": "user", "category": "intent", "content": "Fix the bug"},
                {"role": "ai", "category": "reasoning", "content": "Found the issue"},
                {"role": "ai", "category": "reasoning", "content": "Applied fix"}
            ]
        });

        let result = execute(&agit_dir, Some(args));
        assert!(result.is_error.is_none());
        assert!(get_text(&result.content[0]).contains("3 entries"));
    }

    #[test]
    fn test_log_step_batch_empty() {
        let temp = setup_agit_dir();
        let agit_dir = temp.path().join(".agit");

        let args = json!({
            "batch": []
        });

        let result = execute(&agit_dir, Some(args));
        assert!(result.is_error.is_none());
        assert!(get_text(&result.content[0]).contains("No entries"));
    }

    #[test]
    fn test_log_step_batch_partial_errors() {
        let temp = setup_agit_dir();
        let agit_dir = temp.path().join(".agit");

        let args = json!({
            "batch": [
                {"role": "user", "category": "intent", "content": "Valid entry"},
                {"role": "invalid", "category": "intent", "content": "Invalid role"},
                {"role": "ai", "category": "reasoning", "content": "Another valid"}
            ]
        });

        let result = execute(&agit_dir, Some(args));
        // Should succeed with partial errors
        assert!(result.is_error.is_none());
        let text = get_text(&result.content[0]);
        assert!(text.contains("2 entries"));
        assert!(text.contains("error"));
    }

    #[test]
    fn test_log_step_missing_params() {
        let temp = setup_agit_dir();
        let agit_dir = temp.path().join(".agit");

        // Neither batch nor single-entry params
        let args = json!({
            "role": "user"
        });

        let result = execute(&agit_dir, Some(args));
        assert_eq!(result.is_error, Some(true));
    }
}
