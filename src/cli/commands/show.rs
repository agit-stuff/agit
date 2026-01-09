//! Implementation of the `agit show` command.

use std::path::Path;

use git2::Repository;

use crate::cli::args::ShowArgs;
use crate::core::{detect_version, ensure_sync, EnsureSyncResult, StorageVersion};
use crate::domain::{BlobContent, ObjectType, WrappedBlob, WrappedNeuralCommit};
use crate::error::{AgitError, Result, StorageError};
use crate::storage::{
    FileHeadStore, FileObjectStore, FileRefStore, GitObjectStore, GitRefStore, HeadStore,
    ObjectStore, RefStore,
};

/// Execute the `show` command.
pub fn execute(args: ShowArgs) -> Result<()> {
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

    // Detect storage version
    let version = {
        let repo = Repository::discover(&cwd)?;
        detect_version(&agit_dir, &repo)
    };
    let is_v2 = matches!(version, StorageVersion::V2GitNative);

    // Get the commit to show
    let commit_hash = if let Some(hash) = args.hash {
        // Find neural commit by git hash
        find_neural_commit_by_git_hash(&cwd, &agit_dir, &hash, is_v2)?
    } else {
        // Show HEAD
        let head_store = FileHeadStore::new(&agit_dir);
        let branch = head_store.get()?.unwrap_or_else(|| "main".to_string());

        if is_v2 {
            let ref_store = GitRefStore::new(&cwd);
            ref_store
                .get(&branch)?
                .ok_or(AgitError::Storage(StorageError::NotFound {
                    hash: "HEAD".to_string(),
                }))?
        } else {
            let ref_store = FileRefStore::new(&agit_dir);
            ref_store
                .get(&branch)?
                .ok_or(AgitError::Storage(StorageError::NotFound {
                    hash: "HEAD".to_string(),
                }))?
        }
    };

    // Load the commit using appropriate store
    let data = if is_v2 {
        let object_store = GitObjectStore::new(&cwd);
        object_store.load(&commit_hash)?
    } else {
        let object_store = FileObjectStore::new(&agit_dir);
        object_store.load(&commit_hash)?
    };
    let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;
    let commit = wrapped.data;

    // Print commit info
    println!("Neural Commit: {}", commit_hash);
    println!("Git Commit:    {}", commit.git_hash);
    println!("Author:        {}", commit.author);
    println!(
        "Date:          {}",
        commit.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!();
    println!("Summary:");
    println!("  {}", commit.summary);

    // Show roadmap if requested
    if args.roadmap {
        println!();
        println!("Roadmap:");
        if let Ok(roadmap) = load_blob(&cwd, &agit_dir, &commit.roadmap_hash, is_v2) {
            for line in roadmap.content.lines() {
                println!("  {}", line);
            }
        } else {
            println!("  (not found)");
        }
    }

    // Show trace if requested
    if args.trace {
        println!();
        println!("Trace:");
        if let Ok(trace) = load_blob(&cwd, &agit_dir, &commit.trace_hash, is_v2) {
            for line in trace.content.lines() {
                println!("  {}", line);
            }
        } else {
            println!("  (not found)");
        }
    }

    Ok(())
}

/// Find a neural commit by its associated git hash.
fn find_neural_commit_by_git_hash(
    cwd: &Path,
    agit_dir: &Path,
    git_hash: &str,
    is_v2: bool,
) -> Result<String> {
    // Walk through all commits looking for matching git_hash
    // This is O(n) - could be optimized with an index later
    let head_store = FileHeadStore::new(agit_dir);
    let branch = head_store.get()?.unwrap_or_else(|| "main".to_string());

    let mut current: Option<String> = if is_v2 {
        let ref_store = GitRefStore::new(cwd);
        ref_store.get(&branch)?
    } else {
        let ref_store = FileRefStore::new(agit_dir);
        ref_store.get(&branch)?
    };

    while let Some(hash) = current {
        let data = if is_v2 {
            let object_store = GitObjectStore::new(cwd);
            object_store.load(&hash)?
        } else {
            let object_store = FileObjectStore::new(agit_dir);
            object_store.load(&hash)?
        };
        let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;
        let commit = wrapped.data;

        // Check if git hash matches (prefix match)
        if commit.git_hash.starts_with(git_hash) || git_hash.starts_with(&commit.git_hash) {
            return Ok(hash);
        }

        current = commit.first_parent().map(|s| s.to_string());
    }

    Err(AgitError::Storage(StorageError::NotFound {
        hash: git_hash.to_string(),
    }))
}

/// Load a blob from storage.
fn load_blob(cwd: &Path, agit_dir: &Path, hash: &str, is_v2: bool) -> Result<BlobContent> {
    let data = if is_v2 {
        let object_store = GitObjectStore::new(cwd);
        object_store.load(hash)?
    } else {
        let object_store = FileObjectStore::new(agit_dir);
        object_store.load(hash)?
    };
    let wrapped: WrappedBlob = serde_json::from_slice(&data)?;

    if wrapped.object_type != ObjectType::Blob {
        return Err(AgitError::Storage(StorageError::Corrupt {
            hash: hash.to_string(),
            reason: "Expected blob object".to_string(),
        }));
    }

    Ok(wrapped.data)
}
