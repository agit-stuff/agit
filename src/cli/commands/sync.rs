//! Implementation of the `agit sync` command.
//!
//! This command syncs agit state with git state. It's primarily used by
//! git hooks to automatically attach pending thoughts to commits made
//! via `git commit` directly.

use git2::Repository;

use crate::cli::args::SyncArgs;
use crate::core::{
    detect_version, ensure_sync, CommitPipeline, GitNativeCommitPipeline, StorageVersion,
    SynthesizeSummary,
};
use crate::error::Result;
use crate::git::GitRepository;
use crate::storage::{
    FileHeadStore, FileIndexStore, FileObjectStore, FileRefStore, GitObjectStore, GitRefStore,
    IndexStore, ObjectStore, RefStore,
};

/// Execute the `sync` command.
pub fn execute(args: SyncArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    // Check if initialized - silently return if not
    if !agit_dir.exists() {
        return Ok(());
    }

    let hook = args.hook.as_deref();

    match hook {
        Some("post-commit") => sync_post_commit(&cwd, &agit_dir, args.quiet),
        Some("post-checkout") => sync_post_checkout(&cwd, &agit_dir, args.quiet),
        Some("post-merge") => sync_post_merge(&cwd, &agit_dir, args.quiet),
        Some("post-rewrite") => sync_post_rewrite(&cwd, &agit_dir, args.quiet),
        None => sync_all(&cwd, &agit_dir, args.quiet),
        Some(_) => Ok(()), // Unknown hook, ignore
    }
}

/// Sync after `git commit` - link pending thoughts to the new commit.
fn sync_post_commit(cwd: &std::path::Path, agit_dir: &std::path::Path, quiet: bool) -> Result<()> {
    // Read pending entries
    let index_store = FileIndexStore::new(agit_dir);
    let entries = if index_store.has_staged()? {
        index_store.read_staged()?
    } else {
        index_store.read_all()?
    };

    // No pending thoughts - nothing to link
    if entries.is_empty() {
        return Ok(());
    }

    // Get current git HEAD
    let git = GitRepository::open(cwd)?;
    let git_hash = git.head_commit_hash()?;

    // Detect storage version
    let version = {
        let repo = Repository::discover(cwd)?;
        detect_version(agit_dir, &repo)
    };
    let is_v2 = matches!(version, StorageVersion::V2GitNative);

    // Check if neural commit already exists for this git hash
    // (means `agit commit` was used, not `git commit`)
    let already_linked = if is_v2 {
        let objects = GitObjectStore::new(cwd);
        let refs = GitRefStore::new(cwd);
        find_neural_by_git_hash(&objects, &refs, &git_hash)?
    } else {
        let objects = FileObjectStore::new(agit_dir);
        let refs = FileRefStore::new(agit_dir);
        find_neural_by_git_hash(&objects, &refs, &git_hash)?
    };

    if already_linked.is_some() {
        // Neural commit already exists - agit commit was used
        return Ok(());
    }

    // Synthesize summary from entries
    let summary = SynthesizeSummary::synthesize(&entries);

    // Create neural commit linked to existing git commit
    let result = if is_v2 {
        let mut pipeline = GitNativeCommitPipeline::new(agit_dir.to_path_buf(), git)?;
        pipeline.link_to_existing_commit(&git_hash, &summary)?
    } else {
        let object_store = FileObjectStore::new(agit_dir);
        let ref_store = FileRefStore::new(agit_dir);
        let head_store = FileHeadStore::new(agit_dir);
        let mut pipeline = CommitPipeline::new(
            agit_dir.to_path_buf(),
            git,
            object_store,
            ref_store,
            head_store,
            index_store,
        );
        pipeline.link_to_existing_commit(&git_hash, &summary)?
    };

    if !quiet {
        println!(
            "[agit] Linked {} thought(s) to commit {}",
            entries.len(),
            &result.git_hash[..7.min(result.git_hash.len())]
        );
    }

    Ok(())
}

/// Sync after `git checkout` - ensure branch sync and index stashing.
fn sync_post_checkout(
    cwd: &std::path::Path,
    agit_dir: &std::path::Path,
    quiet: bool,
) -> Result<()> {
    // Call existing ensure_sync which handles branch switching
    if let Some(result) = ensure_sync(cwd, agit_dir)? {
        if !quiet {
            use crate::core::EnsureSyncResult;
            match result {
                EnsureSyncResult::ForkedToNew { new_branch, .. } => {
                    println!("[agit] Synced to new branch: '{}'", new_branch);
                },
                EnsureSyncResult::SwitchedToExisting { new_branch, .. } => {
                    println!("[agit] Restored context for branch: '{}'", new_branch);
                },
                EnsureSyncResult::AlreadyInSync { .. } => {},
            }
        }
    }
    Ok(())
}

/// Sync after `git merge` - link thoughts to merge commit.
fn sync_post_merge(cwd: &std::path::Path, agit_dir: &std::path::Path, quiet: bool) -> Result<()> {
    // Similar to post-commit - link pending thoughts to the merge commit
    sync_post_commit(cwd, agit_dir, quiet)
}

/// Sync after `git rebase` or `git commit --amend` - reconcile rewritten commits.
fn sync_post_rewrite(cwd: &std::path::Path, agit_dir: &std::path::Path, quiet: bool) -> Result<()> {
    // Call ensure_sync which includes rewind/amend detection
    if let Some(result) = ensure_sync(cwd, agit_dir)? {
        if !quiet {
            use crate::core::EnsureSyncResult;
            match result {
                EnsureSyncResult::ForkedToNew { new_branch, .. } => {
                    println!("[agit] Reconciled to branch: '{}'", new_branch);
                },
                EnsureSyncResult::SwitchedToExisting { new_branch, .. } => {
                    println!("[agit] Reconciled to branch: '{}'", new_branch);
                },
                EnsureSyncResult::AlreadyInSync { .. } => {},
            }
        }
    }
    Ok(())
}

/// Full sync - run all reconciliation checks.
fn sync_all(cwd: &std::path::Path, agit_dir: &std::path::Path, quiet: bool) -> Result<()> {
    // First ensure branch sync and rewind detection
    sync_post_checkout(cwd, agit_dir, quiet)?;

    // Then check if there are pending thoughts to link
    sync_post_commit(cwd, agit_dir, quiet)?;

    Ok(())
}

/// Find neural commit hash by git commit hash.
fn find_neural_by_git_hash<O: ObjectStore, R: RefStore>(
    objects: &O,
    refs: &R,
    git_hash: &str,
) -> Result<Option<String>> {
    use crate::domain::WrappedNeuralCommit;

    for branch in refs.list()? {
        if let Some(mut neural_hash) = refs.get(&branch)? {
            let mut visited = std::collections::HashSet::new();
            loop {
                if visited.contains(&neural_hash) {
                    break;
                }
                visited.insert(neural_hash.clone());

                let data = match objects.load(&neural_hash) {
                    Ok(d) => d,
                    Err(_) => break,
                };
                let wrapped: WrappedNeuralCommit = match serde_json::from_slice(&data) {
                    Ok(w) => w,
                    Err(_) => break,
                };

                if wrapped.data.git_hash.starts_with(git_hash)
                    || git_hash.starts_with(&wrapped.data.git_hash)
                {
                    return Ok(Some(neural_hash));
                }

                neural_hash = match wrapped.data.first_parent() {
                    Some(p) => p.to_string(),
                    None => break,
                };
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_git_repo() -> TempDir {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join(".git")).unwrap();
        temp
    }

    #[test]
    fn test_sync_returns_ok_when_not_initialized() {
        let temp = setup_git_repo();
        // No .agit directory
        let args = SyncArgs {
            hook: None,
            quiet: true,
        };

        // Should not error, just return Ok
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();
        let result = execute(args);
        std::env::set_current_dir(cwd).unwrap();

        assert!(result.is_ok());
    }
}
