//! Index (staging area) storage implementation.
//!
//! The index stores entries as JSONL (one JSON object per line).

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::domain::IndexEntry;
use crate::error::{AgitError, IndexError, Result};

use super::IndexStore;

/// File-based index store.
#[derive(Clone)]
pub struct FileIndexStore {
    /// Path to the index file (`.agit/index`).
    index_path: PathBuf,
}

impl FileIndexStore {
    /// Create a new file index store.
    pub fn new(agit_dir: &Path) -> Self {
        Self {
            index_path: agit_dir.join("index"),
        }
    }

    /// Ensure the index file exists.
    pub fn ensure_exists(&self) -> Result<()> {
        if !self.index_path.exists() {
            File::create(&self.index_path)?;
        }
        Ok(())
    }

    /// Get the path to the staged-index file.
    fn staged_path(&self) -> PathBuf {
        self.index_path.with_file_name("staged-index")
    }

    /// Get the path to the stash directory for a specific branch.
    fn stash_path(&self, branch: &str) -> PathBuf {
        // Go up from .agit/index to .agit/, then into stash/<branch>/index
        self.index_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("stash")
            .join(branch)
            .join("index")
    }

    /// Stash the current index to a branch-specific location.
    ///
    /// This is called when switching branches to preserve the pending thoughts
    /// for the old branch. The index is copied to `.agit/stash/<branch>/index`.
    ///
    /// # Arguments
    ///
    /// * `branch` - The branch name to stash the index for
    ///
    /// # Returns
    ///
    /// `true` if the index was stashed, `false` if there was nothing to stash.
    pub fn stash_to_branch(&self, branch: &str) -> Result<bool> {
        if !self.index_path.exists() || self.count()? == 0 {
            return Ok(false); // Nothing to stash
        }

        let stash_path = self.stash_path(branch);

        // Create the stash directory if it doesn't exist
        if let Some(parent) = stash_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Copy the index to the stash location
        fs::copy(&self.index_path, &stash_path)?;

        Ok(true)
    }

    /// Restore the index from a branch-specific stash location.
    ///
    /// This is called when switching to a branch to restore its pending thoughts.
    /// If a stash exists for the branch, it's copied to `.agit/index`.
    /// If no stash exists, the index is cleared.
    ///
    /// # Arguments
    ///
    /// * `branch` - The branch name to restore the index from
    ///
    /// # Returns
    ///
    /// `true` if a stash was restored, `false` if the index was cleared.
    pub fn restore_from_branch(&self, branch: &str) -> Result<bool> {
        let stash_path = self.stash_path(branch);

        if stash_path.exists() {
            // Restore the stash to the main index
            fs::copy(&stash_path, &self.index_path)?;
            Ok(true)
        } else {
            // No stash for this branch - clear the index
            self.clear()?;
            Ok(false)
        }
    }
}

impl IndexStore for FileIndexStore {
    fn append(&self, entry: &IndexEntry) -> Result<()> {
        let json = serde_json::to_string(entry)?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.index_path)
            .map_err(|e| {
                AgitError::Index(IndexError::AppendFailed(format!(
                    "Failed to open index: {}",
                    e
                )))
            })?;

        writeln!(file, "{}", json).map_err(|e| {
            AgitError::Index(IndexError::AppendFailed(format!(
                "Failed to write entry: {}",
                e
            )))
        })?;

        Ok(())
    }

    fn read_all(&self) -> Result<Vec<IndexEntry>> {
        if !self.index_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.index_path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let entry: IndexEntry = serde_json::from_str(&line).map_err(|e| {
                AgitError::Index(IndexError::MalformedEntry {
                    line: line_num + 1,
                    reason: e.to_string(),
                })
            })?;

            entries.push(entry);
        }

        Ok(entries)
    }

    fn clear(&self) -> Result<()> {
        if self.index_path.exists() {
            fs::write(&self.index_path, "").map_err(|e| {
                AgitError::Index(IndexError::ClearFailed(format!(
                    "Failed to clear index: {}",
                    e
                )))
            })?;
        }
        Ok(())
    }

    fn count(&self) -> Result<usize> {
        if !self.index_path.exists() {
            return Ok(0);
        }

        let file = File::open(&self.index_path)?;
        let reader = BufReader::new(file);

        Ok(reader
            .lines()
            .map_while(|l| l.ok())
            .filter(|l| !l.trim().is_empty())
            .count())
    }

    fn freeze(&self) -> Result<()> {
        if self.index_path.exists() && self.count()? > 0 {
            fs::copy(&self.index_path, self.staged_path())?;
            self.clear()?;
        }
        Ok(())
    }

    fn has_staged(&self) -> Result<bool> {
        Ok(self.staged_path().exists())
    }

    fn read_staged(&self) -> Result<Vec<IndexEntry>> {
        let staged_path = self.staged_path();
        if !staged_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&staged_path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let entry: IndexEntry = serde_json::from_str(&line).map_err(|e| {
                AgitError::Index(IndexError::MalformedEntry {
                    line: line_num + 1,
                    reason: e.to_string(),
                })
            })?;

            entries.push(entry);
        }

        Ok(entries)
    }

    fn clear_staged(&self) -> Result<()> {
        let staged_path = self.staged_path();
        if staged_path.exists() {
            fs::remove_file(&staged_path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FileIndexStore) {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();
        let store = FileIndexStore::new(&agit_dir);
        (temp, store)
    }

    #[test]
    fn test_append_and_read() {
        let (_temp, store) = setup();

        let entry1 = IndexEntry::user_intent("Fix the bug");
        let entry2 = IndexEntry::ai_reasoning("Add try/catch");

        store.append(&entry1).unwrap();
        store.append(&entry2).unwrap();

        let entries = store.read_all().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "Fix the bug");
        assert_eq!(entries[1].content, "Add try/catch");
    }

    #[test]
    fn test_count() {
        let (_temp, store) = setup();

        assert_eq!(store.count().unwrap(), 0);
        assert!(store.is_empty().unwrap());

        store.append(&IndexEntry::user_intent("First")).unwrap();
        assert_eq!(store.count().unwrap(), 1);

        store.append(&IndexEntry::user_intent("Second")).unwrap();
        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn test_clear() {
        let (_temp, store) = setup();

        store.append(&IndexEntry::user_intent("Test")).unwrap();
        assert_eq!(store.count().unwrap(), 1);

        store.clear().unwrap();
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn test_read_empty() {
        let (_temp, store) = setup();
        let entries = store.read_all().unwrap();
        assert!(entries.is_empty());
    }
}
