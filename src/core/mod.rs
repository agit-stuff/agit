//! Core business logic for AGIT.
//!
//! This module contains the main algorithms and pipelines,
//! separated from I/O concerns for better testability.

use std::path::Path;

use crate::error::Result;

mod branch_sync;
mod commit_pipeline;
mod migration;
pub mod reconcile;
mod synthesizer;

pub use branch_sync::*;
pub use commit_pipeline::*;
pub use migration::*;
pub use synthesizer::*;

/// Ensure AGIT is synced with the current Git branch.
///
/// This is a convenience function to call at the start of CLI commands.
/// Returns `None` if already in sync (no action needed), or `Some(result)`
/// if sync was performed.
///
/// # Arguments
///
/// * `project_root` - Path to the project root (where .git is)
/// * `agit_dir` - Path to the .agit directory
pub fn ensure_sync(project_root: &Path, agit_dir: &Path) -> Result<Option<EnsureSyncResult>> {
    let sync = BranchSync::new(project_root, agit_dir)?;
    let result = sync.ensure_branch_sync()?;

    match result {
        EnsureSyncResult::AlreadyInSync { .. } => Ok(None),
        _ => Ok(Some(result)),
    }
}
