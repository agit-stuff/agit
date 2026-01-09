//! Implementation of the `agit status` command.

use git2::Repository;

use crate::cli::args::StatusArgs;
use crate::core::{detect_version, ensure_sync, EnsureSyncResult, StorageVersion};
use crate::error::{AgitError, Result};
use crate::git::GitRepository;
use crate::storage::{
    FileHeadStore, FileIndexStore, FileRefStore, GitRefStore, HeadStore, IndexStore, RefStore,
};

/// Execute the `status` command.
pub fn execute(args: StatusArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    // Check if initialized
    if !agit_dir.exists() {
        return Err(AgitError::NotInitialized);
    }

    // Ensure branch sync
    if let Some(result) = ensure_sync(&cwd, &agit_dir)? {
        match &result {
            EnsureSyncResult::ForkedToNew { new_branch, .. } => {
                println!("Syncing Agit memory to new branch: '{}'", new_branch);
            }
            EnsureSyncResult::SwitchedToExisting { new_branch, .. } => {
                println!("Syncing Agit memory to branch: '{}'", new_branch);
            }
            _ => {}
        }
    }

    // Get Git branch
    let git_repo = GitRepository::open(&cwd)?;
    let git_branch = git_repo.current_branch()?;

    // Get AGIT branch (HEAD)
    let head_store = FileHeadStore::new(&agit_dir);
    let agit_branch = head_store.get()?.unwrap_or_else(|| "main".to_string());

    // Check sync status
    let in_sync = git_branch == agit_branch;

    // Get pending entries count
    let index_store = FileIndexStore::new(&agit_dir);
    let pending_count = index_store.count()?;

    // Get staged context count
    let staged_count = if index_store.has_staged()? {
        index_store.read_staged()?.len()
    } else {
        0
    };

    // Detect storage version and get latest neural commit
    let version = {
        let repo = Repository::discover(&cwd)?;
        detect_version(&agit_dir, &repo)
    };

    let latest_hash: Option<String> = if matches!(version, StorageVersion::V2GitNative) {
        let ref_store = GitRefStore::new(&cwd);
        ref_store.get(&agit_branch)?
    } else {
        let ref_store = FileRefStore::new(&agit_dir);
        ref_store.get(&agit_branch)?
    };

    // Print status
    println!("On branch {}", git_branch);

    if !in_sync {
        println!("  (AGIT branch: {} - out of sync!)", agit_branch);
    }

    println!();

    if staged_count > 0 {
        println!(
            "Staged context: {} thought(s) ready for commit",
            staged_count
        );
        println!("  (use \"agit commit\" to create commit)");
    }

    if pending_count > 0 {
        println!("Pending thoughts: {}", pending_count);
        println!("  (will be included in next \"agit add\")");
    } else if staged_count == 0 {
        println!("No pending thoughts.");
        println!("  (use \"agit record\" to add thoughts)");
    }

    if args.verbose {
        println!();
        if let Some(hash) = latest_hash {
            println!("Latest neural commit: {}", &hash[..7.min(hash.len())]);
        } else {
            println!("No neural commits yet.");
        }
    }

    Ok(())
}
