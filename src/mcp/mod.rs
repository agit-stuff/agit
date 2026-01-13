//! MCP (Model Context Protocol) server module.
//!
//! This module implements the MCP server that AI editors (Cursor, Claude Code,
//! Windsurf, etc.) can connect to for logging context and reading project state.
//!
//! # Architecture
//!
//! The MCP server uses JSON-RPC 2.0 over stdio:
//! - AI editors connect to the server via stdin/stdout
//! - They can call tools like `agit_log_step` to record context
//! - They can read resources like `agit://history/recent`
//! - The server responds with JSON-RPC responses
//!
//! # Available Tools
//!
//! - `agit_log_step` - Log a step (intent/reasoning) in the conversation
//! - `agit_read_roadmap` - Read the project roadmap
//! - `agit_get_context` - Get context for a specific git commit
//!
//! # Available Resources
//!
//! - `agit://history/recent` - Recent commit summaries

pub mod protocol;
pub mod resources;
pub mod server;
pub mod tools;

pub use server::McpServer;
