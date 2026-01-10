//! Implementation of the `agit reset` command.

use std::fs;
use std::io::{self, IsTerminal, Write};

use crate::cli::args::ResetArgs;
use crate::error::{AgitError, Result};
use crate::storage::{FileIndexStore, IndexStore};

/// Execute the `reset` command.
pub fn execute(args: ResetArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    // Check if initialized
    if !agit_dir.exists() {
        return Err(AgitError::NotInitialized);
    }

    let index_store = FileIndexStore::new(&agit_dir);

    // Gather counts for display
    let pending_count = index_store.count()?;
    let staged_count = if index_store.has_staged()? {
        index_store.read_staged()?.len()
    } else {
        0
    };

    // Determine what will be cleared based on flags
    let (clear_pending, clear_staged, clear_stashes) = if args.hard {
        (true, true, true)
    } else if args.soft {
        (true, false, false)
    } else {
        // Default: clear pending and staged
        (true, true, false)
    };

    // Count stashes if --hard
    let stash_count = if clear_stashes {
        count_stashes(&agit_dir)?
    } else {
        0
    };

    // Calculate what will actually be cleared
    let effective_pending = if clear_pending { pending_count } else { 0 };
    let effective_staged = if clear_staged { staged_count } else { 0 };
    let effective_stashes = if clear_stashes { stash_count } else { 0 };

    // Check if there's anything to clear
    let total = effective_pending + effective_staged + effective_stashes;
    if total == 0 {
        println!("Nothing to reset.");
        return Ok(());
    }

    // Show what will be cleared
    println!("The following will be cleared:");
    println!();
    if clear_pending && pending_count > 0 {
        println!("  Pending thoughts:  {} entries", pending_count);
        show_entries_preview(&index_store.read_all()?, "    ");
    }
    if clear_staged && staged_count > 0 {
        println!("  Staged context:    {} entries", staged_count);
        show_entries_preview(&index_store.read_staged()?, "    ");
    }
    if clear_stashes && stash_count > 0 {
        println!("  Branch stashes:    {} branches", stash_count);
    }
    println!();

    // Confirm unless --yes or non-interactive
    if !args.yes {
        if io::stdin().is_terminal() {
            let action = if args.hard { "hard reset" } else { "reset" };
            print!("Proceed with {}? [y/N]: ", action);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => {},
                _ => {
                    println!("Reset cancelled.");
                    return Ok(());
                },
            }
        } else {
            // Non-interactive without --yes: require explicit flag
            return Err(AgitError::InvalidArgument(
                "Reset requires --yes flag in non-interactive mode.".to_string(),
            ));
        }
    }

    // Perform the reset
    if clear_pending {
        index_store.clear()?;
    }
    if clear_staged {
        index_store.clear_staged()?;
    }
    if clear_stashes {
        clear_all_stashes(&agit_dir)?;
    }

    // Report what was done
    println!("Reset complete:");
    if clear_pending && pending_count > 0 {
        println!("  Cleared {} pending thought(s)", pending_count);
    }
    if clear_staged && staged_count > 0 {
        println!("  Cleared {} staged thought(s)", staged_count);
    }
    if clear_stashes && stash_count > 0 {
        println!("  Cleared stashes for {} branch(es)", stash_count);
    }

    Ok(())
}

/// Count the number of branch stashes.
fn count_stashes(agit_dir: &std::path::Path) -> Result<usize> {
    let stash_dir = agit_dir.join("stash");
    if !stash_dir.exists() {
        return Ok(0);
    }

    let count = fs::read_dir(&stash_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .count();

    Ok(count)
}

/// Clear all branch stashes.
fn clear_all_stashes(agit_dir: &std::path::Path) -> Result<()> {
    let stash_dir = agit_dir.join("stash");
    if stash_dir.exists() {
        fs::remove_dir_all(&stash_dir)?;
    }
    Ok(())
}

/// Show a preview of entries (first few, truncated).
fn show_entries_preview(entries: &[crate::domain::IndexEntry], indent: &str) {
    let max_preview = 3;
    for (i, entry) in entries.iter().take(max_preview).enumerate() {
        let content_preview = if entry.content.len() > 50 {
            format!("{}...", &entry.content[..50])
        } else {
            entry.content.clone()
        };
        println!("{}{}: \"{}\"", indent, entry.category, content_preview);
        if i == max_preview - 1 && entries.len() > max_preview {
            println!("{}... and {} more", indent, entries.len() - max_preview);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();
        fs::write(agit_dir.join("index"), "").unwrap();
        temp
    }

    #[test]
    fn test_count_stashes_empty() {
        let temp = setup();
        let agit_dir = temp.path().join(".agit");

        let count = count_stashes(&agit_dir).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_stashes() {
        let temp = setup();
        let agit_dir = temp.path().join(".agit");

        // Create some stashes
        let stash_dir = agit_dir.join("stash");
        fs::create_dir_all(stash_dir.join("main")).unwrap();
        fs::create_dir_all(stash_dir.join("feature")).unwrap();

        let count = count_stashes(&agit_dir).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_clear_stashes() {
        let temp = setup();
        let agit_dir = temp.path().join(".agit");

        // Create a stash
        let stash_dir = agit_dir.join("stash").join("main");
        fs::create_dir_all(&stash_dir).unwrap();
        fs::write(stash_dir.join("index"), "test").unwrap();

        clear_all_stashes(&agit_dir).unwrap();

        assert!(!agit_dir.join("stash").exists());
    }

    #[test]
    fn test_clear_stashes_no_dir() {
        let temp = setup();
        let agit_dir = temp.path().join(".agit");

        // Should not error if stash dir doesn't exist
        let result = clear_all_stashes(&agit_dir);
        assert!(result.is_ok());
    }
}
