//! Implementation of the `agit show` command.

use crate::cli::args::ShowArgs;
use crate::core::{ensure_sync, EnsureSyncResult};
use crate::domain::{BlobContent, ObjectType, WrappedBlob, WrappedNeuralCommit};
use crate::error::{AgitError, Result, StorageError};
use crate::storage::{
    FileHeadStore, FileObjectStore, FileRefStore, HeadStore, ObjectStore, RefStore,
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

    let object_store = FileObjectStore::new(&agit_dir);

    // Get the commit to show
    let commit_hash = if let Some(hash) = args.hash {
        // Find neural commit by git hash
        find_neural_commit_by_git_hash(&agit_dir, &hash)?
    } else {
        // Show HEAD
        let head_store = FileHeadStore::new(&agit_dir);
        let branch = head_store.get()?.unwrap_or_else(|| "main".to_string());

        let ref_store = FileRefStore::new(&agit_dir);
        ref_store
            .get(&branch)?
            .ok_or(AgitError::Storage(StorageError::NotFound {
                hash: "HEAD".to_string(),
            }))?
    };

    // Load the commit
    let data = object_store.load(&commit_hash)?;
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
        if let Ok(roadmap) = load_blob(&object_store, &commit.roadmap_hash) {
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
        if let Ok(trace) = load_blob(&object_store, &commit.trace_hash) {
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
fn find_neural_commit_by_git_hash(agit_dir: &std::path::Path, git_hash: &str) -> Result<String> {
    // Walk through all commits looking for matching git_hash
    // This is O(n) - could be optimized with an index later
    let head_store = FileHeadStore::new(agit_dir);
    let branch = head_store.get()?.unwrap_or_else(|| "main".to_string());

    let ref_store = FileRefStore::new(agit_dir);
    let object_store = FileObjectStore::new(agit_dir);

    let mut current = ref_store.get(&branch)?;

    while let Some(hash) = current {
        let data = object_store.load(&hash)?;
        let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;
        let commit = wrapped.data;

        // Check if git hash matches (prefix match)
        if commit.git_hash.starts_with(git_hash) || git_hash.starts_with(&commit.git_hash) {
            return Ok(hash);
        }

        current = commit.parent_hash;
    }

    Err(AgitError::Storage(StorageError::NotFound {
        hash: git_hash.to_string(),
    }))
}

/// Load a blob from storage.
fn load_blob(store: &FileObjectStore, hash: &str) -> Result<BlobContent> {
    let data = store.load(hash)?;
    let wrapped: WrappedBlob = serde_json::from_slice(&data)?;

    if wrapped.object_type != ObjectType::Blob {
        return Err(AgitError::Storage(StorageError::Corrupt {
            hash: hash.to_string(),
            reason: "Expected blob object".to_string(),
        }));
    }

    Ok(wrapped.data)
}
