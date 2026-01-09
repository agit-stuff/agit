//! Git refs-based reference storage implementation.
//!
//! Stores branch references in the `refs/agit/heads/` namespace.
//! This namespace is invisible in `git branch -a` and GitHub/GitLab UI.

use std::path::{Path, PathBuf};

use git2::{ErrorCode, Oid, Repository};

use crate::error::{AgitError, Result, StorageError};

use super::RefStore;

/// Prefix for all Agit refs in the Git refs namespace.
const AGIT_REF_PREFIX: &str = "refs/agit/heads/";

/// Git refs-based reference store.
///
/// Branch references are stored as `refs/agit/heads/{branch}`.
/// This namespace is invisible to standard Git tooling.
pub struct GitRefStore {
    /// Path to the git repository root.
    repo_path: PathBuf,
}

impl GitRefStore {
    /// Create a new Git ref store.
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

    /// Get the full ref name for a branch.
    fn full_ref_name(branch: &str) -> String {
        format!("{}{}", AGIT_REF_PREFIX, branch)
    }
}

impl RefStore for GitRefStore {
    fn get(&self, ref_name: &str) -> Result<Option<String>> {
        let repo = self.repo()?;
        let full_name = Self::full_ref_name(ref_name);

        let result = match repo.find_reference(&full_name) {
            Ok(reference) => {
                let oid = reference.target().ok_or_else(|| {
                    AgitError::Storage(StorageError::Corrupt {
                        hash: ref_name.to_string(),
                        reason: "Reference has no target".to_string(),
                    })
                })?;
                Ok(Some(oid.to_string()))
            },
            Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
            Err(e) => Err(AgitError::Git(e)),
        };
        result
    }

    fn update(&self, ref_name: &str, hash: &str) -> Result<()> {
        let repo = self.repo()?;
        let full_name = Self::full_ref_name(ref_name);

        let oid = Oid::from_str(hash)
            .map_err(|_| AgitError::Storage(StorageError::InvalidHash(hash.to_string())))?;

        repo.reference(
            &full_name,
            oid,
            true, // force update
            &format!("agit: update {}", ref_name),
        )?;

        Ok(())
    }

    fn delete(&self, ref_name: &str) -> Result<()> {
        let repo = self.repo()?;
        let full_name = Self::full_ref_name(ref_name);

        let result = match repo.find_reference(&full_name) {
            Ok(mut reference) => {
                reference.delete()?;
                Ok(())
            },
            Err(e) if e.code() == ErrorCode::NotFound => Ok(()),
            Err(e) => Err(AgitError::Git(e)),
        };
        result
    }

    fn list(&self) -> Result<Vec<String>> {
        let repo = self.repo()?;
        let pattern = format!("{}*", AGIT_REF_PREFIX);

        let refs = repo.references_glob(&pattern)?;
        let mut branches = Vec::new();

        for reference in refs {
            let reference = reference?;
            if let Some(name) = reference.name() {
                if let Some(branch) = name.strip_prefix(AGIT_REF_PREFIX) {
                    branches.push(branch.to_string());
                }
            }
        }

        branches.sort();
        Ok(branches)
    }
}

// GitRefStore is Send because PathBuf is Send.
// It's also Sync because we open a new Repository for each operation.
unsafe impl Sync for GitRefStore {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, GitRefStore, Repository) {
        let temp = TempDir::new().unwrap();
        let repo = Repository::init(temp.path()).unwrap();

        // Create an initial commit so we have something to reference
        {
            let sig = repo
                .signature()
                .unwrap_or_else(|_| git2::Signature::now("Test", "test@test.com").unwrap());
            let tree_id = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        let store = GitRefStore::new(temp.path());
        (temp, store, repo)
    }

    #[test]
    fn test_get_nonexistent() {
        let (_temp, store, _) = setup();
        let result = store.get("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_and_get() {
        let (_temp, store, repo) = setup();

        // Create a blob to get a valid OID
        let oid = repo.blob(b"test content").unwrap();
        let hash = oid.to_string();

        // Update ref
        store.update("main", &hash).unwrap();

        // Get ref
        let result = store.get("main").unwrap();
        assert_eq!(result, Some(hash));
    }

    #[test]
    fn test_delete() {
        let (_temp, store, repo) = setup();

        let oid = repo.blob(b"test content").unwrap();
        let hash = oid.to_string();

        store.update("to-delete", &hash).unwrap();
        assert!(store.get("to-delete").unwrap().is_some());

        store.delete("to-delete").unwrap();
        assert!(store.get("to-delete").unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let (_temp, store, _) = setup();
        // Should not error
        store.delete("nonexistent").unwrap();
    }

    #[test]
    fn test_list() {
        let (_temp, store, repo) = setup();

        let oid = repo.blob(b"test content").unwrap();
        let hash = oid.to_string();

        store.update("main", &hash).unwrap();
        store.update("feature", &hash).unwrap();
        store.update("develop", &hash).unwrap();

        let branches = store.list().unwrap();
        assert_eq!(branches, vec!["develop", "feature", "main"]);
    }

    #[test]
    fn test_list_empty() {
        let (_temp, store, _) = setup();
        let branches = store.list().unwrap();
        assert!(branches.is_empty());
    }

    #[test]
    fn test_ref_is_invisible() {
        let (_temp, store, repo) = setup();

        let oid = repo.blob(b"test content").unwrap();
        let hash = oid.to_string();

        store.update("test-branch", &hash).unwrap();

        // Verify it's NOT visible as a regular branch
        let branches: Vec<_> = repo
            .branches(None)
            .unwrap()
            .filter_map(|b| b.ok())
            .filter_map(|(b, _)| b.name().ok().flatten().map(String::from))
            .collect();

        // refs/agit/* should not appear in git branch list
        assert!(!branches.iter().any(|b| b.contains("test-branch")));

        // But it should be visible via our store
        assert!(store.get("test-branch").unwrap().is_some());
    }
}
