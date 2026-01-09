//! Branch reference storage implementation.
//!
//! Refs are stored as files in `.agit/refs/heads/{branch}`,
//! each containing the hash of the latest neural commit.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::safety::atomic_write;

use super::{HeadStore, RefStore};

/// File-based reference store.
pub struct FileRefStore {
    /// Path to the refs directory (`.agit/refs`).
    refs_dir: PathBuf,
}

impl FileRefStore {
    /// Create a new file ref store.
    pub fn new(agit_dir: &Path) -> Self {
        Self {
            refs_dir: agit_dir.join("refs"),
        }
    }

    /// Get the path to a ref file.
    fn ref_path(&self, ref_name: &str) -> PathBuf {
        self.refs_dir.join("heads").join(ref_name)
    }

    /// Ensure the refs/heads directory exists.
    pub fn ensure_exists(&self) -> Result<()> {
        fs::create_dir_all(self.refs_dir.join("heads"))?;
        Ok(())
    }
}

impl RefStore for FileRefStore {
    fn get(&self, ref_name: &str) -> Result<Option<String>> {
        let path = self.ref_path(ref_name);
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;
        Ok(Some(content.trim().to_string()))
    }

    fn update(&self, ref_name: &str, hash: &str) -> Result<()> {
        let path = self.ref_path(ref_name);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        atomic_write(&path, hash.as_bytes())?;
        Ok(())
    }

    fn delete(&self, ref_name: &str) -> Result<()> {
        let path = self.ref_path(ref_name);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn list(&self) -> Result<Vec<String>> {
        let heads_dir = self.refs_dir.join("heads");
        if !heads_dir.exists() {
            return Ok(Vec::new());
        }

        let mut refs = Vec::new();
        for entry in fs::read_dir(&heads_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    refs.push(name.to_string());
                }
            }
        }

        refs.sort();
        Ok(refs)
    }
}

/// File-based HEAD store.
pub struct FileHeadStore {
    /// Path to the HEAD file (`.agit/HEAD`).
    head_path: PathBuf,
}

impl FileHeadStore {
    /// Create a new file HEAD store.
    pub fn new(agit_dir: &Path) -> Self {
        Self {
            head_path: agit_dir.join("HEAD"),
        }
    }

    /// Ensure the HEAD file exists with a default value.
    pub fn ensure_exists(&self, default_branch: &str) -> Result<()> {
        if !self.head_path.exists() {
            fs::write(&self.head_path, default_branch)?;
        }
        Ok(())
    }
}

impl HeadStore for FileHeadStore {
    fn get(&self) -> Result<Option<String>> {
        if !self.head_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.head_path)?;
        Ok(Some(content.trim().to_string()))
    }

    fn set(&self, branch: &str) -> Result<()> {
        atomic_write(&self.head_path, branch.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FileRefStore, FileHeadStore) {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();
        let refs = FileRefStore::new(&agit_dir);
        let head = FileHeadStore::new(&agit_dir);
        refs.ensure_exists().unwrap();
        (temp, refs, head)
    }

    #[test]
    fn test_ref_update_and_get() {
        let (_temp, refs, _head) = setup();

        refs.update("main", "abc123").unwrap();

        let hash = refs.get("main").unwrap();
        assert_eq!(hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_ref_not_found() {
        let (_temp, refs, _head) = setup();

        let hash = refs.get("nonexistent").unwrap();
        assert_eq!(hash, None);
    }

    #[test]
    fn test_ref_list() {
        let (_temp, refs, _head) = setup();

        refs.update("main", "hash1").unwrap();
        refs.update("feature", "hash2").unwrap();
        refs.update("develop", "hash3").unwrap();

        let list = refs.list().unwrap();
        assert_eq!(list, vec!["develop", "feature", "main"]);
    }

    #[test]
    fn test_ref_delete() {
        let (_temp, refs, _head) = setup();

        refs.update("main", "abc123").unwrap();
        assert!(refs.get("main").unwrap().is_some());

        refs.delete("main").unwrap();
        assert!(refs.get("main").unwrap().is_none());
    }

    #[test]
    fn test_head_store() {
        let (_temp, _refs, head) = setup();

        head.ensure_exists("main").unwrap();
        assert_eq!(head.get().unwrap(), Some("main".to_string()));

        head.set("feature").unwrap();
        assert_eq!(head.get().unwrap(), Some("feature".to_string()));
    }
}
