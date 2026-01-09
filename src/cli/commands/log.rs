//! Implementation of the `agit log` command.

use crate::cli::args::LogArgs;
use crate::core::{ensure_sync, EnsureSyncResult};
use crate::domain::{NeuralCommit, ObjectType, WrappedNeuralCommit};
use crate::error::{AgitError, Result};
use crate::storage::{
    FileHeadStore, FileObjectStore, FileRefStore, HeadStore, ObjectStore, RefStore,
};

/// Execute the `log` command.
pub fn execute(args: LogArgs) -> Result<()> {
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

    // Get current branch
    let head_store = FileHeadStore::new(&agit_dir);
    let branch = head_store.get()?.unwrap_or_else(|| "main".to_string());

    // Get latest commit hash
    let ref_store = FileRefStore::new(&agit_dir);
    let mut current_hash = ref_store.get(&branch)?;

    if current_hash.is_none() {
        println!("No neural commits yet on branch '{}'.", branch);
        return Ok(());
    }

    // Walk the commit chain
    let object_store = FileObjectStore::new(&agit_dir);
    let mut count = 0;

    while let Some(hash) = current_hash {
        if count >= args.count {
            break;
        }

        // Load the commit
        let data = object_store.load(&hash)?;
        let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;

        if wrapped.object_type != ObjectType::NeuralCommit {
            break;
        }

        let commit = wrapped.data;

        if args.oneline {
            print_oneline(&commit);
        } else {
            print_full(&commit);
        }

        // Use first_parent() to walk the main line (handles both old and new format)
        current_hash = commit.first_parent().map(|s| s.to_string());
        count += 1;
    }

    Ok(())
}

/// Print a commit in oneline format.
fn print_oneline(commit: &NeuralCommit) {
    let short_hash = commit.short_hash();
    let summary = truncate(&commit.summary, 60);
    println!("{} {}", short_hash, summary);
}

/// Print a commit in full format.
fn print_full(commit: &NeuralCommit) {
    println!("commit {} (git: {})", commit.short_hash(), commit.git_hash);

    // Show merge information if this is a merge commit
    if commit.is_merge() {
        let parents: Vec<&str> = commit
            .parents()
            .iter()
            .map(|p| if p.len() >= 7 { &p[..7] } else { *p })
            .collect();
        println!("Merge:  {}", parents.join(" "));
    }

    println!("Author: {}", commit.author);
    println!(
        "Date:   {}",
        commit.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!();
    println!("    {}", commit.summary);
    println!();
}

/// Truncate a string to a maximum length.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a long string", 10), "this is...");
    }
}
