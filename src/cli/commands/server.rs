//! Implementation of the `agit server` command.
//!
//! This starts the MCP (Model Context Protocol) server that AI assistants
//! can connect to for logging thoughts and reading context.

use crate::cli::args::ServerArgs;
use crate::error::{AgitError, Result};
use crate::mcp::McpServer;

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
        eprintln!("Listening on stdio for JSON-RPC requests...");
    }

    // Initialize tracing if verbose
    if args.verbose {
        use tracing_subscriber::EnvFilter;
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("agit=debug")),
            )
            .with_writer(std::io::stderr)
            .init();
    }

    // Create and run the MCP server
    let server = McpServer::new(cwd, args.verbose);
    server.run()?;

    Ok(())
}
