//! MCP Server implementation.
//!
//! This module implements the MCP (Model Context Protocol) server
//! that listens on stdio for JSON-RPC requests from AI editors.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use serde_json::{json, Value};
use tracing::{debug, error, info};

use crate::error::Result;
use crate::mcp::protocol::*;
use crate::mcp::tools;

/// MCP Server that handles JSON-RPC requests over stdio.
pub struct McpServer {
    /// Path to the project root.
    project_root: PathBuf,
    /// Path to the .agit directory.
    agit_dir: PathBuf,
    /// Whether verbose logging is enabled.
    #[allow(dead_code)]
    verbose: bool,
}

impl McpServer {
    /// Create a new MCP server.
    pub fn new(project_root: PathBuf, verbose: bool) -> Self {
        let agit_dir = project_root.join(".agit");
        Self {
            project_root,
            agit_dir,
            verbose,
        }
    }

    /// Run the server, reading from stdin and writing to stdout.
    pub fn run(&self) -> Result<()> {
        info!("AGIT MCP server starting...");
        info!("Project root: {}", self.project_root.display());

        let stdin = std::io::stdin();
        let reader = BufReader::new(stdin.lock());
        let mut stdout = std::io::stdout();

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to read from stdin: {}", e);
                    break;
                },
            };

            if line.is_empty() {
                continue;
            }

            debug!("Received: {}", line);

            let response = self.handle_message(&line);

            if let Some(resp) = response {
                let resp_str = serde_json::to_string(&resp).unwrap_or_default();
                debug!("Sending: {}", resp_str);

                if let Err(e) = writeln!(stdout, "{}", resp_str) {
                    error!("Failed to write response: {}", e);
                    break;
                }

                if let Err(e) = stdout.flush() {
                    error!("Failed to flush stdout: {}", e);
                    break;
                }
            }
        }

        info!("AGIT MCP server shutting down");
        Ok(())
    }

    /// Handle a single JSON-RPC message.
    fn handle_message(&self, message: &str) -> Option<JsonRpcResponse> {
        // Parse the JSON-RPC request
        let request: JsonRpcRequest = match serde_json::from_str(message) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse request: {}", e);
                return Some(JsonRpcResponse::error(
                    None,
                    PARSE_ERROR,
                    &format!("Parse error: {}", e),
                ));
            },
        };

        // Handle the method
        let result = self.dispatch(&request);

        // Return response (only if there was an id - notifications don't get responses)
        if request.id.is_some() {
            Some(match result {
                Ok(value) => JsonRpcResponse::success(request.id, value),
                Err((code, msg)) => JsonRpcResponse::error(request.id, code, &msg),
            })
        } else {
            // This is a notification, no response needed
            None
        }
    }

    /// Dispatch a request to the appropriate handler.
    fn dispatch(&self, request: &JsonRpcRequest) -> std::result::Result<Value, (i32, String)> {
        match request.method.as_str() {
            // MCP lifecycle methods
            "initialize" => self.handle_initialize(request.params.as_ref()),
            "initialized" => Ok(json!(null)), // Notification, just acknowledge
            "shutdown" => Ok(json!(null)),

            // MCP tool methods
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(request.params.as_ref()),

            // Unknown method
            _ => {
                error!("Unknown method: {}", request.method);
                Err((
                    METHOD_NOT_FOUND,
                    format!("Method not found: {}", request.method),
                ))
            },
        }
    }

    /// Handle the initialize request.
    fn handle_initialize(
        &self,
        _params: Option<&Value>,
    ) -> std::result::Result<Value, (i32, String)> {
        info!("Client initializing");

        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability {
                    list_changed: false,
                },
            },
            server_info: ServerInfo {
                name: "agit".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        serde_json::to_value(result).map_err(|e| (INTERNAL_ERROR, e.to_string()))
    }

    /// Handle the tools/list request.
    fn handle_tools_list(&self) -> std::result::Result<Value, (i32, String)> {
        let tools = vec![
            ToolDefinition {
                name: "agit_log_step".to_string(),
                description: "BATCH MODE ONLY. Call ONCE at task completion with 'batch' parameter. IMPORTANT: Include 'locations' array for file-specific reasoning - memories without locations matching staged files are AUTO-PRUNED on commit (Strict Binding). FORBIDDEN: calling mid-task or without batch.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "role": {
                            "type": "string",
                            "enum": ["user", "ai"],
                            "description": "DEPRECATED - use batch instead"
                        },
                        "category": {
                            "type": "string",
                            "enum": ["intent", "reasoning", "error"],
                            "description": "DEPRECATED - use batch instead"
                        },
                        "content": {
                            "type": "string",
                            "description": "DEPRECATED - use batch instead"
                        },
                        "batch": {
                            "type": "array",
                            "description": "REQUIRED. Array of {role, category, content, locations?} entries.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "role": {
                                        "type": "string",
                                        "enum": ["user", "ai"]
                                    },
                                    "category": {
                                        "type": "string",
                                        "enum": ["intent", "reasoning", "error"]
                                    },
                                    "content": {
                                        "type": "string"
                                    },
                                    "locations": {
                                        "type": "array",
                                        "description": "IMPORTANT: Include file locations for Strict Binding. Memories without locations matching staged files are auto-pruned on commit.",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "file": {
                                                    "type": "string",
                                                    "description": "Relative file path from repo root (e.g., 'src/auth.rs')"
                                                },
                                                "start_line": {
                                                    "type": "integer",
                                                    "description": "Starting line number (1-indexed)"
                                                },
                                                "end_line": {
                                                    "type": "integer",
                                                    "description": "Ending line number (inclusive)"
                                                }
                                            },
                                            "required": ["file"]
                                        }
                                    }
                                },
                                "required": ["role", "category", "content"]
                            }
                        }
                    }
                }),
            },
            ToolDefinition {
                name: "agit_read_roadmap".to_string(),
                description: "Read project goals and roadmap. Call this when starting work or when uncertain about project direction.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            ToolDefinition {
                name: "agit_get_context".to_string(),
                description: "Retrieve AI reasoning context for a git commit. Use this to understand WHY past changes were made.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "git_hash": {
                            "type": "string",
                            "description": "The git commit hash to look up"
                        }
                    },
                    "required": ["git_hash"]
                }),
            },
            ToolDefinition {
                name: "agit_get_recent_summaries".to_string(),
                description: "Get recent commit summaries to understand what was done recently. Call this when starting work or when uncertain about recent changes.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "count": {
                            "type": "integer",
                            "description": "Number of recent summaries to return (default: 5)"
                        }
                    },
                    "required": []
                }),
            },
            ToolDefinition {
                name: "agit_get_relevant_context".to_string(),
                description: "Search past reasoning logs for relevant context. Use this to find WHY past decisions were made based on keywords.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query to find relevant past reasoning"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results (default: 5)"
                        }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "agit_get_file_history".to_string(),
                description: "Get the history of changes to a specific file. Returns summaries of neural commits that touched this file. Use this BEFORE modifying any file to understand past changes.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "filepath": {
                            "type": "string",
                            "description": "The file path to get history for (e.g., 'src/auth.rs')"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of commits to return (default: 3)"
                        }
                    },
                    "required": ["filepath"]
                }),
            },
        ];

        let result = ToolsListResult { tools };
        serde_json::to_value(result).map_err(|e| (INTERNAL_ERROR, e.to_string()))
    }

    /// Handle the tools/call request.
    fn handle_tools_call(
        &self,
        params: Option<&Value>,
    ) -> std::result::Result<Value, (i32, String)> {
        let params = params.ok_or((INVALID_PARAMS, "Missing params".to_string()))?;

        let call_params: ToolCallParams = serde_json::from_value(params.clone())
            .map_err(|e| (INVALID_PARAMS, format!("Invalid params: {}", e)))?;

        let result = match call_params.name.as_str() {
            "agit_log_step" => tools::log_step::execute(&self.agit_dir, call_params.arguments),
            "agit_read_roadmap" => tools::read_roadmap::execute(&self.agit_dir),
            "agit_get_context" => tools::get_context::execute(
                &self.project_root,
                &self.agit_dir,
                call_params.arguments,
            ),
            "agit_get_recent_summaries" => tools::get_recent_summaries::execute(
                &self.project_root,
                &self.agit_dir,
                call_params.arguments,
            ),
            "agit_get_relevant_context" => {
                tools::relevant_context::execute(&self.agit_dir, call_params.arguments)
            },
            "agit_get_file_history" => tools::get_file_history::execute(
                &self.project_root,
                &self.agit_dir,
                call_params.arguments,
            ),
            _ => ToolCallResult::error(&format!("Unknown tool: {}", call_params.name)),
        };

        serde_json::to_value(result).map_err(|e| (INTERNAL_ERROR, e.to_string()))
    }
}
