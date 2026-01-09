//! Implementation of the `agit record` command.

use std::path::Path;

use crate::cli::args::RecordArgs;
use crate::domain::{Category, IndexEntry, Role};
use crate::error::{AgitError, Result};
use crate::storage::{FileIndexStore, IndexStore};

/// Execute the `record` command.
pub fn execute(args: RecordArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    // Check if initialized
    if !agit_dir.exists() {
        return Err(AgitError::NotInitialized);
    }

    // Determine role and category
    let role = if args.ai { Role::Ai } else { Role::User };
    let category = if args.intent {
        Category::Intent
    } else if args.ai {
        Category::Reasoning
    } else {
        Category::Note
    };

    // Create and append entry
    let entry = IndexEntry::new(role, category, &args.message);
    append_entry(&agit_dir, &entry)?;

    // Print confirmation
    let role_str = match role {
        Role::User => "user",
        Role::Ai => "ai",
    };
    let category_str = match category {
        Category::Intent => "intent",
        Category::Reasoning => "reasoning",
        Category::Error => "error",
        Category::Note => "note",
    };

    println!("Recorded {} {} to staging area", role_str, category_str);

    Ok(())
}

/// Append an entry to the index.
fn append_entry(agit_dir: &Path, entry: &IndexEntry) -> Result<()> {
    let index_store = FileIndexStore::new(agit_dir);
    index_store.append(entry)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();
        fs::write(agit_dir.join("index"), "").unwrap();
        temp
    }

    #[test]
    fn test_append_entry() {
        let temp = setup();
        let agit_dir = temp.path().join(".agit");

        let entry = IndexEntry::user_note("Test note");
        append_entry(&agit_dir, &entry).unwrap();

        let index_store = FileIndexStore::new(&agit_dir);
        let entries = index_store.read_all().unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "Test note");
    }
}
