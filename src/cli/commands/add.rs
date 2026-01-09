//! Implementation of the `agit add` command.

use crate::cli::args::AddArgs;
use crate::error::{AgitError, Result};
use crate::git::GitRepository;
use crate::storage::{FileIndexStore, IndexStore};

/// Execute the `add` command.
pub fn execute(args: AddArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    if !agit_dir.exists() {
        return Err(AgitError::NotInitialized);
    }

    let git_repo = GitRepository::open(&cwd)?;
    let pathspecs: Vec<&str> = args.pathspec.iter().map(|s| s.as_str()).collect();
    let count = git_repo.stage_files(&pathspecs)?;

    if count == 0 {
        println!("No files to add.");
        return Ok(());
    }

    let index_store = FileIndexStore::new(&agit_dir);
    let context_count = index_store.count()?;
    index_store.freeze()?;

    println!("\nStaged {} file(s) for commit.", count);
    if context_count > 0 {
        println!("Frozen {} thought(s) for next commit.", context_count);
    }

    Ok(())
}
