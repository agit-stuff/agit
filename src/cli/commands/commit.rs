//! Implementation of the `agit commit` command.

use crate::cli::args::CommitArgs;
use crate::core::{CommitPipeline, SynthesizeSummary};
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

    // Check if there are entries in the index
    let index_store = FileIndexStore::new(&agit_dir);
    let entries = index_store.read_all()?;

    if entries.is_empty() && !args.amend {
        println!("No thoughts recorded in staging area.");
        println!("Use 'agit record' to add thoughts, or the MCP server will log them automatically.");
        return Ok(());
    }

    // Get commit message
    let message = match args.message {
        Some(msg) => msg,
        None => {
            return Err(AgitError::InvalidArgument(
                "Commit message required. Use -m or --message".to_string(),
            ));
        }
    };

    // Create the pipeline
    let git_repo = GitRepository::open(&cwd)?;
    let object_store = FileObjectStore::new(&agit_dir);
    let ref_store = FileRefStore::new(&agit_dir);
    let head_store = FileHeadStore::new(&agit_dir);

    let mut pipeline = CommitPipeline::new(
        agit_dir,
        git_repo,
        object_store,
        ref_store,
        head_store,
        index_store,
    );

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

    println!("Created neural commit: {}", result.neural_hash[..7].to_string());
    println!("Linked to git commit:  {}", result.git_hash[..7].to_string());
    println!();
    println!("Summary: {}", final_summary);

    Ok(())
}
