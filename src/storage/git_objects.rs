//! Git ODB-based object storage implementation.
//!
//! Stores objects as Git blobs in the repository's object database.
//! This makes Agit objects invisible in `git status` and standard workflows.

use std::path::{Path, PathBuf};

use git2::{Oid, Repository};

use crate::error::{AgitError, Result, StorageError};

use super::ObjectStore;

/// Git ODB-based object store.
///
/// Objects are stored as Git blobs, identified by their SHA-1 OID.
/// This is invisible to `git status` and standard Git workflows.
pub struct GitObjectStore {
    /// Path to the git repository root.
    repo_path: PathBuf,
}

impl GitObjectStore {
    /// Create a new Git object store.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the git repository root (where .git is)
    pub fn new(repo_path: &Path) -> Self {
        Self {
            repo_path: repo_path.to_path_buf(),
        }
    }

    /// Open the repository.
    fn repo(&self) -> Result<Repository> {
        Repository::open(&self.repo_path).map_err(AgitError::Git)
    }
}

impl ObjectStore for GitObjectStore {
    fn save(&self, content: &[u8]) -> Result<String> {
        let repo = self.repo()?;
        let oid = repo.blob(content)?;
        Ok(oid.to_string())
    }

    fn load(&self, hash: &str) -> Result<Vec<u8>> {
        let repo = self.repo()?;

        let oid = Oid::from_str(hash)
            .map_err(|_| AgitError::Storage(StorageError::InvalidHash(hash.to_string())))?;

        let blob = repo.find_blob(oid).map_err(|e| {
            if e.code() == git2::ErrorCode::NotFound {
                AgitError::Storage(StorageError::NotFound {
                    hash: hash.to_string(),
                })
            } else {
                AgitError::Git(e)
            }
        })?;

        Ok(blob.content().to_vec())
    }

    fn exists(&self, hash: &str) -> Result<bool> {
        let repo = self.repo()?;

        let oid = match Oid::from_str(hash) {
            Ok(oid) => oid,
            Err(_) => return Ok(false),
        };

        let exists = repo.find_blob(oid).is_ok();
        Ok(exists)
    }

    fn delete(&self, _hash: &str) -> Result<()> {
        // Git objects are garbage collected, not explicitly deleted.
        // Use `git gc` to clean up unreferenced objects.
        Ok(())
    }
}

// GitObjectStore is Send because PathBuf is Send.
// It's also Sync because we open a new Repository for each operation.
unsafe impl Sync for GitObjectStore {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, GitObjectStore) {
        let temp = TempDir::new().unwrap();
        // Initialize a git repository
        Repository::init(temp.path()).unwrap();
        let store = GitObjectStore::new(temp.path());
        (temp, store)
    }

    #[test]
    fn test_save_and_load() {
        let (_temp, store) = setup();
        let content = b"test content for git blob";

        let hash = store.save(content).unwrap();
        // Git SHA-1 = 40 hex chars
        assert_eq!(hash.len(), 40);

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
    fn test_exists() {
        let (_temp, store) = setup();
        let content = b"existence check";

        // Before save
        let hash = "0000000000000000000000000000000000000000";
        assert!(!store.exists(hash).unwrap());

        // After save
        let hash = store.save(content).unwrap();
        assert!(store.exists(&hash).unwrap());
    }

    #[test]
    fn test_load_not_found() {
        let (_temp, store) = setup();

        let result = store.load("0000000000000000000000000000000000000000");
        assert!(matches!(
            result,
            Err(AgitError::Storage(StorageError::NotFound { .. }))
        ));
    }

    #[test]
    fn test_delete_is_noop() {
        let (_temp, store) = setup();
        let content = b"to be deleted";

        let hash = store.save(content).unwrap();
        // Delete is a no-op for Git objects
        store.delete(&hash).unwrap();
        // Object still exists (Git GC handles cleanup)
        assert!(store.exists(&hash).unwrap());
    }
}
