//! Implementation of the `agit commit` command.

use std::io::{self, IsTerminal, Write};

use git2::Repository;

use crate::cli::args::CommitArgs;
use crate::core::{
    check_conflicted_state, detect_version, ensure_sync, sanitize_entries, ChangeState,
    CommitPipeline, EnsureSyncResult, GitNativeCommitPipeline, StorageVersion, SynthesizeSummary,
};
use crate::error::{AgitError, Result};
use crate::git::GitRepository;
use crate::storage::{FileHeadStore, FileIndexStore, FileObjectStore, FileRefStore, IndexStore};

/// Check if stdin is connected to an interactive terminal.
fn is_interactive() -> bool {
    io::stdin().is_terminal()
}

/// Execute the `commit` command.
pub fn execute(args: CommitArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    // Check if initialized
    if !agit_dir.exists() {
        return Err(AgitError::NotInitialized);
    }

    // Check for merge/rebase in progress
    let git_repo = GitRepository::open(&cwd)?;
    check_conflicted_state(&git_repo)?;

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
    let mut entries = if has_staged {
        index_store.read_staged()?
    } else {
        pending_entries.clone()
    };

    // === AUTO-PRUNING: Strict Binding Enforcement ===
    // Only apply when NOT using --journal flag (journal = explicit memory-only intent)
    if !args.journal && !entries.is_empty() {
        // Get list of files staged for this commit
        let staged_files = git_repo.staged_files()?;

        // Sanitize entries against staged files
        let sanitize_result = sanitize_entries(entries, &staged_files);

        // Print warnings for pruned entries
        if !sanitize_result.pruned.is_empty() {
            eprintln!();
            for (pruned_entry, orphaned_paths) in &sanitize_result.pruned {
                for path in orphaned_paths {
                    eprintln!("  Pruning orphaned memory for reverted file: {}", path);
                }
                // Log the pruned content (truncated)
                let content_preview = if pruned_entry.content.len() > 60 {
                    format!("{}...", &pruned_entry.content[..60])
                } else {
                    pruned_entry.content.clone()
                };
                eprintln!(
                    "     \u{2514} {}: \"{}\"",
                    pruned_entry.category, content_preview
                );
            }
            eprintln!();
            eprintln!(
                "  {} memory entries pruned (files not staged)",
                sanitize_result.pruned.len()
            );
            eprintln!();
        }

        // Replace entries with sanitized (kept) entries
        entries = sanitize_result.kept;

        // Check for "empty" guard condition
        if entries.is_empty() && staged_files.is_empty() {
            return Err(AgitError::InvalidArgument(
                "No effective changes detected. Memories for reverted files were pruned.\n\
                 Use `agit commit --journal` to force a context-only commit."
                    .to_string(),
            ));
        }
    }
    // === END AUTO-PRUNING ===

    if entries.is_empty() && !args.amend && !args.journal {
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

    // Handle Memory-Only state with Conscious Commit protocol
    if change_state == ChangeState::MemoryOnly {
        if args.journal || args.yes {
            // Case 1: User explicitly passed --journal flag (or --yes for backward compat)
            // ✅ PROCEED - This is an intentional decision
            println!("[Agit] Creating Journal Entry (memory-only commit)...");
        } else if is_interactive() {
            // Case 2: Interactive terminal without flag
            // 🛑 INTERCEPT - Prompt for confirmation
            println!();
            println!("⚠️  No code changes detected. This will be saved as a 'Journal Entry'");
            println!("    (Empty Commit) to preserve the decision history.");
            println!();
            print!("Are you sure? [y/N]: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => {
                    // User confirmed - treat as --journal
                    println!();
                    println!("[Agit] Creating Journal Entry (memory-only commit)...");
                },
                _ => {
                    // User declined or invalid input
                    println!();
                    println!("Commit cancelled.");
                    println!(
                        "Tip: Use 'agit commit --journal' to explicitly create a Journal Entry."
                    );
                    return Ok(());
                },
            }
        } else {
            // Case 3: Non-interactive (script/agent) without --journal flag
            // ❌ ABORT - Require explicit flag
            return Err(AgitError::InvalidArgument(
                "No code changes detected. To save a pure thought/decision, use `agit commit --journal`.".to_string()
            ));
        }
    }

    // Handle NoChanges state (no code AND no memory)
    if change_state == ChangeState::NoChanges {
        if args.journal {
            // User explicitly wants a truly empty journal entry
            println!("[Agit] Creating empty Journal Entry (decision checkpoint)...");
        } else {
            return Err(AgitError::NothingToCommit);
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
