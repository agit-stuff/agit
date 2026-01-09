//! Implementation of the `agit commit` command.

use std::io::{self, Write};

use crate::cli::args::CommitArgs;
use crate::core::{ensure_sync, ChangeState, CommitPipeline, EnsureSyncResult, SynthesizeSummary};
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
            }
            EnsureSyncResult::SwitchedToExisting { new_branch, .. } => {
                println!("Syncing Agit memory to branch: '{}'", new_branch);
            }
            _ => {}
        }
    }

    // Check if there are entries in the index
    let index_store = FileIndexStore::new(&agit_dir);
    let entries = index_store.read_all()?;

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

    // Create the pipeline
    let git_repo = GitRepository::open(&cwd)?;
    let object_store = FileObjectStore::new(&agit_dir);
    let ref_store = FileRefStore::new(&agit_dir);
    let head_store = FileHeadStore::new(&agit_dir);

    let mut pipeline = CommitPipeline::new(
        agit_dir.clone(),
        git_repo,
        object_store,
        ref_store,
        head_store,
        index_store.clone(),
    );

    // Check change state for Intent Check
    let change_state = pipeline.detect_change_state()?;

    // Handle Memory-Only state with Intent Check prompt
    if change_state == ChangeState::MemoryOnly {
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
            }
            "2" => {
                // Discard thoughts
                index_store.clear()?;
                println!();
                println!("Thoughts discarded. Index cleared.");
                return Ok(());
            }
            "3" | "" => {
                // Cancel
                println!();
                println!("Commit cancelled.");
                return Ok(());
            }
            _ => {
                println!();
                println!("Invalid choice. Commit cancelled.");
                return Ok(());
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

    // Execute the commit pipeline
    let result = pipeline.execute(&message, &final_summary)?;

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
