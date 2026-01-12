//! MCP (Model Context Protocol) types.
//!
//! This module defines the JSON-RPC types used for MCP communication.
//! MCP uses JSON-RPC 2.0 over stdio.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Create a success response.
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: Option<Value>, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        }
    }
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// Standard JSON-RPC error codes
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

/// MCP Initialize request parameters.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

/// Client capabilities.
#[derive(Debug, Deserialize, Default)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub roots: Option<RootsCapability>,
    #[serde(default)]
    pub sampling: Option<Value>,
}

/// Roots capability.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootsCapability {
    #[serde(default)]
    pub list_changed: bool,
}

/// Client info.
#[derive(Debug, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// MCP Initialize response result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

/// Server capabilities.
#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
}

/// Tools capability.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    pub list_changed: bool,
}

/// Server info.
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Tool definition.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Tools list response.
#[derive(Debug, Serialize)]
pub struct ToolsListResult {
    pub tools: Vec<ToolDefinition>,
}

/// Tool call request parameters.
#[derive(Debug, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<Value>,
}

/// Tool call result.
#[derive(Debug, Serialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Tool content item.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
}

impl ToolCallResult {
    /// Create a success result with text content.
    pub fn text(content: &str) -> Self {
        Self {
            content: vec![ToolContent::Text {
                text: content.to_string(),
            }],
            is_error: None,
        }
    }

    /// Create an error result.
    pub fn error(message: &str) -> Self {
        Self {
            content: vec![ToolContent::Text {
                text: message.to_string(),
            }],
            is_error: Some(true),
        }
    }
}

// Tool-specific parameter types

/// A code location for MCP input.
/// Represents a file with optional line range.
#[derive(Debug, Clone, Deserialize)]
pub struct LocationInput {
    /// Relative file path from repository root.
    pub file: String,
    /// Starting line number (1-indexed).
    #[serde(default)]
    pub start_line: Option<u32>,
    /// Ending line number (inclusive).
    #[serde(default)]
    pub end_line: Option<u32>,
}

/// A single log entry for batch logging.
#[derive(Debug, Deserialize)]
pub struct LogEntry {
    pub role: String,
    pub category: String,
    pub content: String,
    /// Code locations this entry relates to.
    /// Each location can specify a file and optional line range.
    #[serde(default)]
    pub locations: Option<Vec<LocationInput>>,
    // --- Legacy fields for backward compatibility ---
    /// (Deprecated) Use `locations` instead. Optional file path.
    #[serde(default)]
    pub file_path: Option<String>,
    /// (Deprecated) Use `locations` instead. Optional line number.
    #[serde(default)]
    pub line_number: Option<u32>,
}

/// Parameters for agit_log_step tool.
///
/// Supports both single-entry mode (backward compatible) and batch mode.
#[derive(Debug, Deserialize)]
pub struct LogStepParams {
    /// Single entry role (for backward compatibility)
    #[serde(default)]
    pub role: Option<String>,
    /// Single entry category (for backward compatibility)
    #[serde(default)]
    pub category: Option<String>,
    /// Single entry content (for backward compatibility)
    #[serde(default)]
    pub content: Option<String>,
    /// Code locations for single entry mode.
    #[serde(default)]
    pub locations: Option<Vec<LocationInput>>,
    // --- Legacy fields for backward compatibility ---
    /// (Deprecated) Use `locations` instead. Optional file path.
    #[serde(default)]
    pub file_path: Option<String>,
    /// (Deprecated) Use `locations` instead. Optional line number.
    #[serde(default)]
    pub line_number: Option<u32>,
    /// Optional target repository path. If specified, agit validates that this
    /// matches the current repository before logging. Helps prevent cross-repo
    /// contamination when AI agents work across multiple repos.
    #[serde(default)]
    pub repo_path: Option<String>,
    /// Batch of entries (preferred for multiple logs)
    #[serde(default)]
    pub batch: Option<Vec<LogEntry>>,
}

/// Parameters for agit_get_context tool.
#[derive(Debug, Deserialize)]
pub struct GetContextParams {
    pub git_hash: String,
}

/// Parameters for agit_get_recent_summaries tool.
#[derive(Debug, Deserialize)]
pub struct GetRecentSummariesParams {
    /// Number of recent summaries to return (default: 5).
    pub count: Option<usize>,
}
