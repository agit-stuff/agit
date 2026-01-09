//! Implementation of the `agit server` command.
//!
//! This starts the MCP (Model Context Protocol) server that AI assistants
//! can connect to for logging thoughts and reading context.

use crate::cli::args::ServerArgs;
use crate::error::{AgitError, Result};

/// Execute the `server` command.
pub fn execute(args: ServerArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    // Check if initialized
    if !agit_dir.exists() {
        return Err(AgitError::NotInitialized);
    }

    if args.verbose {
        eprintln!("Starting AGIT MCP server...");
        eprintln!("Project: {}", cwd.display());
    }

    // Run the MCP server
    // For now, this is a placeholder - the full implementation
    // will use tokio and jsonrpc-core for the stdio transport

    // TODO: Implement full MCP server
    // The server should:
    // 1. Listen on stdin for JSON-RPC requests
    // 2. Handle tool calls (agit_log_step, agit_read_roadmap, etc.)
    // 3. Write responses to stdout

    eprintln!("MCP server is not yet fully implemented.");
    eprintln!("For now, use 'agit record' to manually log thoughts.");

    Ok(())
}
