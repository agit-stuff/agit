//! Content-addressable storage implementation.
//!
//! Objects are stored by their SHA-256 hash, with the first two
//! characters used as a subdirectory (like Git).

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{AgitError, Result, StorageError};
use crate::safety::atomic_write;

use super::ObjectStore;

/// File-system based content-addressable store.
pub struct FileObjectStore {
    /// Path to the objects directory (`.agit/objects`).
    objects_dir: PathBuf,
}

impl FileObjectStore {
    /// Create a new file object store.
    pub fn new(agit_dir: &Path) -> Self {
        Self {
            objects_dir: agit_dir.join("objects"),
        }
    }

    /// Compute the SHA-256 hash of content.
    pub fn hash_content(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    /// Get the path where an object would be stored.
    fn object_path(&self, hash: &str) -> Result<PathBuf> {
        if hash.len() < 4 {
            return Err(AgitError::Storage(StorageError::InvalidHash(
                hash.to_string(),
            )));
        }
        let (prefix, rest) = hash.split_at(2);
        Ok(self.objects_dir.join(prefix).join(rest))
    }

    /// Ensure the parent directory for an object exists.
    fn ensure_parent_dir(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

impl ObjectStore for FileObjectStore {
    fn save(&self, content: &[u8]) -> Result<String> {
        let hash = Self::hash_content(content);
        let path = self.object_path(&hash)?;

        // Don't write if already exists (content-addressable = idempotent)
        if path.exists() {
            return Ok(hash);
        }

        self.ensure_parent_dir(&path)?;

        // Use atomic write to prevent partial writes
        atomic_write(&path, content)?;

        Ok(hash)
    }

    fn load(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.object_path(hash)?;

        if !path.exists() {
            return Err(AgitError::Storage(StorageError::NotFound {
                hash: hash.to_string(),
            }));
        }

        fs::read(&path).map_err(|e| {
            AgitError::Storage(StorageError::ReadFailed(format!(
                "Failed to read {}: {}",
                hash, e
            )))
        })
    }

    fn exists(&self, hash: &str) -> Result<bool> {
        let path = self.object_path(hash)?;
        Ok(path.exists())
    }

    fn delete(&self, hash: &str) -> Result<()> {
        let path = self.object_path(hash)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FileObjectStore) {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();
        let store = FileObjectStore::new(&agit_dir);
        (temp, store)
    }

    #[test]
    fn test_hash_content() {
        let hash = FileObjectStore::hash_content(b"hello world");
        assert_eq!(hash.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
    }

    #[test]
    fn test_save_and_load() {
        let (_temp, store) = setup();
        let content = b"test content";

        let hash = store.save(content).unwrap();
        assert!(store.exists(&hash).unwrap());

        let loaded = store.load(&hash).unwrap();
        assert_eq!(loaded, content);
    }

    #[test]
    fn test_save_is_idempotent() {
        let (_temp, store) = setup();
        let content = b"same content";

        let hash1 = store.save(content).unwrap();
        let hash2 = store.save(content).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_load_not_found() {
        let (_temp, store) = setup();

        let result = store.load("0000000000000000000000000000000000000000000000000000000000000000");
        assert!(matches!(
            result,
            Err(AgitError::Storage(StorageError::NotFound { .. }))
        ));
    }

    #[test]
    fn test_delete() {
        let (_temp, store) = setup();
        let content = b"to be deleted";

        let hash = store.save(content).unwrap();
        assert!(store.exists(&hash).unwrap());

        store.delete(&hash).unwrap();
        assert!(!store.exists(&hash).unwrap());
    }
}
