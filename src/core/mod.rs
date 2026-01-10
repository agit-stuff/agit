//! Core business logic for AGIT.
//!
//! This module contains the main algorithms and pipelines,
//! separated from I/O concerns for better testability.

use std::path::Path;

use crate::error::{AgitError, Result};
use crate::git::GitRepository;
use crate::storage::{FileObjectStore, FileRefStore, GitObjectStore, GitRefStore};

mod branch_sync;
mod commit_pipeline;
mod migration;
pub mod reconcile;
mod synthesizer;

pub use branch_sync::*;
pub use commit_pipeline::*;
pub use migration::*;
pub use reconcile::RewindResult;
pub use synthesizer::*;

/// Ensure AGIT is synced with the current Git branch.
///
/// This is a convenience function to call at the start of CLI commands.
/// Returns `None` if already in sync (no action needed), or `Some(result)`
/// if sync was performed.
///
/// This function performs two types of reconciliation:
/// 1. **Branch sync**: Ensures AGIT HEAD follows the Git branch
/// 2. **Rewind check**: Detects if Git has rewound (e.g., `git reset --hard`)
///    and snaps AGIT back to a valid ancestor
///
/// # Arguments
///
/// * `project_root` - Path to the project root (where .git is)
/// * `agit_dir` - Path to the .agit directory
pub fn ensure_sync(project_root: &Path, agit_dir: &Path) -> Result<Option<EnsureSyncResult>> {
    let sync = BranchSync::new(project_root, agit_dir)?;
    let result = sync.ensure_branch_sync()?;

    // Get the current branch for rewind check
    let branch = match &result {
        EnsureSyncResult::AlreadyInSync { branch } => branch.clone(),
        EnsureSyncResult::SwitchedToExisting { new_branch, .. } => new_branch.clone(),
        EnsureSyncResult::ForkedToNew { new_branch, .. } => new_branch.clone(),
    };

    // Check for rewind (git reset --hard) and snap back if needed
    check_rewind(project_root, agit_dir, &branch)?;

    match result {
        EnsureSyncResult::AlreadyInSync { .. } => Ok(None),
        _ => Ok(Some(result)),
    }
}

/// Check if Git is in a conflicted state (merge or rebase in progress).
///
/// This should be called at the start of mutating commands (record, add, commit)
/// to prevent graph corruption during Git operations.
///
/// # Arguments
///
/// * `git` - Git repository wrapper
///
/// # Returns
///
/// `Ok(())` if not in a conflicted state, `Err(AgitError::ConflictedState)` otherwise.
pub fn check_conflicted_state(git: &GitRepository) -> Result<()> {
    if git.is_merging()? {
        return Err(AgitError::ConflictedState {
            operation: "Merge".to_string(),
        });
    }
    if git.is_rebasing()? {
        return Err(AgitError::ConflictedState {
            operation: "Rebase".to_string(),
        });
    }
    Ok(())
}

/// Check if Git has rewound and snap AGIT back to a valid ancestor.
///
/// This handles the "dangling head" scenario where the user runs `git reset --hard`
/// and Git moves backward, but AGIT still points to neural commits attached to
/// git commits that no longer exist.
fn check_rewind(project_root: &Path, agit_dir: &Path, branch: &str) -> Result<()> {
    use git2::Repository;

    let git = GitRepository::open(project_root)?;

    // Detect storage version
    let repo = Repository::discover(project_root)?;
    let is_v2 = matches!(detect_version(agit_dir, &repo), StorageVersion::V2GitNative);

    let rewind_result = if is_v2 {
        let objects = GitObjectStore::new(project_root);
        let refs = GitRefStore::new(project_root);
        reconcile::reconcile_rewind(&git, &objects, &refs, branch)?
    } else {
        let objects = FileObjectStore::new(agit_dir);
        let refs = FileRefStore::new(agit_dir);
        reconcile::reconcile_rewind(&git, &objects, &refs, branch)?
    };

    match rewind_result {
        RewindResult::Rewound { orphaned_count, .. } => {
            eprintln!("Warning: Git history rewound. Snapped agit HEAD back to valid ancestor.");
            if orphaned_count > 0 {
                eprintln!(
                    "  {} orphaned neural commit(s) left as objects (not deleted).",
                    orphaned_count
                );
            }
        },
        RewindResult::NoValidAncestor { .. } => {
            eprintln!("Warning: Git history rewound but no valid agit ancestor found.");
            eprintln!("  This branch's neural history may be orphaned.");
        },
        RewindResult::NoRewindNeeded => {
            // Normal case - do nothing
        },
    }

    Ok(())
}
