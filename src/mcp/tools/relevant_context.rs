//! Implementation of the agit_get_relevant_context MCP tool.
//!
//! This tool allows AI editors to search for relevant context from
//! past reasoning logs using full-text search.

use std::path::Path;

use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, error};

use crate::mcp::protocol::ToolCallResult;
use crate::search::retriever;

/// Parameters for the agit_get_relevant_context tool.
#[derive(Debug, Deserialize)]
pub struct GetRelevantContextParams {
    /// Search query to find relevant past reasoning.
    pub query: String,
    /// Maximum number of results to return (default: 5).
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    5
}

/// Execute the agit_get_relevant_context tool.
pub fn execute(agit_dir: &Path, arguments: Option<Value>) -> ToolCallResult {
    // Parse arguments
    let args = match arguments {
        Some(v) => v,
        None => {
            return ToolCallResult::error("Missing arguments: 'query' is required");
        },
    };

    let params: GetRelevantContextParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => {
            error!("Invalid params for agit_get_relevant_context: {}", e);
            return ToolCallResult::error(&format!("Invalid parameters: {}", e));
        },
    };

    // Check if agit is initialized
    if !agit_dir.exists() {
        return ToolCallResult::error("AGIT not initialized. Run 'agit init' first.");
    }

    // Perform the search
    match retriever::search(agit_dir, &params.query, params.limit) {
        Ok(results) => {
            if results.is_empty() {
                debug!(
                    "No results found for query '{}' in agit_get_relevant_context",
                    params.query
                );
                return ToolCallResult::text("No relevant context found.");
            }

            // Format results for display
            let formatted: Vec<String> = results
                .iter()
                .map(|r| format!("[{}] (score: {:.2}) {}", r.category, r.score, r.body))
                .collect();

            ToolCallResult::text(&formatted.join("\n\n"))
        },
        Err(e) => {
            error!("Search failed in agit_get_relevant_context: {}", e);
            ToolCallResult::error(&format!("Search failed: {}", e))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Category, IndexEntry, Role};
    use crate::search::indexer::index_entries;
    use chrono::Utc;
    use serde_json::json;
    use tempfile::TempDir;

    fn create_test_entry(content: &str, category: Category) -> IndexEntry {
        IndexEntry {
            role: Role::Ai,
            category,
            content: content.to_string(),
            timestamp: Utc::now(),
            file_path: None,
            line_number: None,
        }
    }

    #[test]
    fn test_get_relevant_context_not_initialized() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");

        let args = json!({
            "query": "authentication"
        });

        let result = execute(&agit_dir, Some(args));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_get_relevant_context_missing_args() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        let result = execute(&agit_dir, None);
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_get_relevant_context_no_results() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        let args = json!({
            "query": "nonexistent"
        });

        let result = execute(&agit_dir, Some(args));
        assert_eq!(result.is_error, None);
        // Should return "No relevant context found." message
    }

    #[test]
    fn test_get_relevant_context_with_results() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        std::fs::create_dir_all(&agit_dir).unwrap();

        // Index some test entries
        let entries = vec![
            create_test_entry("Planning to implement authentication", Category::Intent),
            create_test_entry("Decided to use JWT tokens for auth", Category::Reasoning),
        ];
        index_entries(&agit_dir, &entries).unwrap();

        let args = json!({
            "query": "authentication",
            "limit": 5
        });

        let result = execute(&agit_dir, Some(args));
        assert_eq!(result.is_error, None);
    }
}
