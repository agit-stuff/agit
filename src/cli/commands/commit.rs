//! Implementation of the `agit commit` command.

use std::io::{self, Write};

use git2::Repository;

use crate::cli::args::CommitArgs;
use crate::core::{
    detect_version, ensure_sync, ChangeState, CommitPipeline, EnsureSyncResult,
    GitNativeCommitPipeline, StorageVersion, SynthesizeSummary,
};
use crate::error::{AgitError, Result};
use crate::git::GitRepository;
use crate::storage::{FileHeadStore, FileIndexStore, FileObjectStore, FileRefStore, IndexStore};

/// Execute the `commit` command.
pub fn execute(args: CommitArgs) -> Result<()> {
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
            },
            EnsureSyncResult::SwitchedToExisting { new_branch, .. } => {
                println!("Syncing Agit memory to branch: '{}'", new_branch);
            },
            _ => {},
        }
    }

    // Check if there are entries in the index or staged-index
    let index_store = FileIndexStore::new(&agit_dir);
    let has_staged = index_store.has_staged()?;
    let pending_entries = index_store.read_all()?;

    // Use staged entries if available, otherwise use pending entries
    let entries = if has_staged {
        index_store.read_staged()?
    } else {
        pending_entries.clone()
    };

    if entries.is_empty() && !args.amend {
        println!("No thoughts recorded in staging area.");
        println!(
            "Use 'agit record' to add thoughts, or the MCP server will log them automatically."
        );
        return Ok(());
    }

    // Get commit message
    let message = match args.message {
        Some(msg) => msg,
        None => {
            return Err(AgitError::InvalidArgument(
                "Commit message required. Use -m or --message".to_string(),
            ));
        },
    };

    // Detect storage version
    let version = {
        let repo = Repository::discover(&cwd)?;
        detect_version(&agit_dir, &repo)
    };

    // Use V2 (Git-native) by default for new repos and when detected
    let is_v2 = matches!(version, StorageVersion::V2GitNative);

    // Check change state for Intent Check
    let change_state = if is_v2 {
        let pipeline = GitNativeCommitPipeline::new(agit_dir.clone(), GitRepository::open(&cwd)?)?;
        pipeline.detect_change_state()?
    } else {
        let object_store = FileObjectStore::new(&agit_dir);
        let ref_store = FileRefStore::new(&agit_dir);
        let head_store = FileHeadStore::new(&agit_dir);
        let pipeline = CommitPipeline::new(
            agit_dir.clone(),
            GitRepository::open(&cwd)?,
            object_store,
            ref_store,
            head_store,
            FileIndexStore::new(&agit_dir),
        );
        pipeline.detect_change_state()?
    };

    // Handle Memory-Only state with Intent Check prompt
    if change_state == ChangeState::MemoryOnly {
        if args.yes {
            // Skip prompt in non-interactive mode
            println!("[Agit] Creating plan commit...");
        } else {
            println!();
            println!("Pending thoughts found, but no code changes detected.");
            println!();
            println!("What would you like to do?");
            println!("  [1] Commit as Plan (Save only reasoning to history)");
            println!("  [2] Discard Thoughts (Clear pending thoughts)");
            println!("  [3] Cancel");
            println!();

            print!("Enter choice [1-3]: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            match input.trim() {
                "1" => {
                    // Proceed with memory-only commit
                    println!();
                    println!("[Agit] Creating plan commit...");
                },
                "2" => {
                    // Discard thoughts
                    index_store.clear()?;
                    println!();
                    println!("Thoughts discarded. Index cleared.");
                    return Ok(());
                },
                "3" | "" => {
                    // Cancel
                    println!();
                    println!("Commit cancelled.");
                    return Ok(());
                },
                _ => {
                    println!();
                    println!("Invalid choice. Commit cancelled.");
                    return Ok(());
                },
            }
        }
    }

    // Synthesize summary
    let summary = SynthesizeSummary::synthesize(&entries);
    let final_summary = if args.edit_summary {
        // TODO: Open editor for summary editing
        println!("Summary: {}", summary);
        summary
    } else {
        summary
    };

    // Execute the commit pipeline based on storage version
    let result = if is_v2 {
        let mut pipeline =
            GitNativeCommitPipeline::new(agit_dir.clone(), GitRepository::open(&cwd)?)?;
        pipeline.execute(&message, &final_summary, args.force)?
    } else {
        let object_store = FileObjectStore::new(&agit_dir);
        let ref_store = FileRefStore::new(&agit_dir);
        let head_store = FileHeadStore::new(&agit_dir);
        let mut pipeline = CommitPipeline::new(
            agit_dir.clone(),
            GitRepository::open(&cwd)?,
            object_store,
            ref_store,
            head_store,
            index_store.clone(),
        );
        pipeline.execute(&message, &final_summary, args.force)?
    };

    // Show commit result
    if result.is_memory_only {
        println!("[Agit] Memory-only commit (no code changes)");
    }

    if result.git_commit_created {
        println!("Created git commit:    {}", &result.git_hash[..7]);
    } else {
        println!("Linked to git commit:  {}", &result.git_hash[..7]);
    }
    println!("Created neural commit: {}", &result.neural_hash[..7]);
    println!();
    println!("Summary: {}", final_summary);

    Ok(())
}
