//! AGIT: AI-Native Git Wrapper
//!
//! AGIT captures the "Context" (Why/How) alongside the "Code" (What).
//! It maintains a "Neural Graph" parallel to Git's commit graph.
//!
//! # Philosophy
//!
//! "Code is the Artifact. Context is the Source."
//!
//! # Architecture
//!
//! AGIT uses the "Seamless Echo Strategy" where AI editors (Cursor, Claude, etc.)
//! push context to AGIT via MCP, eliminating the need for LLM API keys in the CLI.
//!
//! # Example
//!
//! ```bash
//! # Initialize AGIT in a git repository
//! agit init
//!
//! # Record a thought manually
//! agit record "Planning to refactor the auth module"
//!
//! # Commit with context
//! agit commit -m "Refactor auth module"
//!
//! # View history with context
//! agit log
//! ```

pub mod cli;
pub mod core;
pub mod domain;
pub mod error;
pub mod git;
pub mod mcp;
pub mod safety;
pub mod storage;
pub mod templates;
pub mod tui;

// Re-export commonly used types
pub use domain::{Category, IndexEntry, NeuralCommit, Role};
pub use error::{AgitError, Result};
