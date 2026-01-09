//! Git repository wrapper.

use std::path::Path;

use git2::Repository;

use crate::error::Result;

/// Wrapper around git2::Repository for Git operations.
pub struct GitRepository {
    repo: Repository,
}

impl GitRepository {
    /// Open a Git repository at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let repo = Repository::discover(path)?;
        Ok(Self { repo })
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> Result<String> {
        let head = self.repo.head()?;

        if head.is_branch() {
            if let Some(name) = head.shorthand() {
                return Ok(name.to_string());
            }
        }

        // Detached HEAD - return the short hash
        if let Some(oid) = head.target() {
            return Ok(oid.to_string()[..7].to_string());
        }

        Ok("unknown".to_string())
    }

    /// Get the hash of the HEAD commit.
    pub fn head_commit_hash(&self) -> Result<String> {
        let head = self.repo.head()?;
        let commit = head.peel_to_commit()?;
        Ok(commit.id().to_string())
    }

    /// Get the user's email from git config.
    pub fn config_user_email(&self) -> Result<Option<String>> {
        let config = self.repo.config()?;
        match config.get_string("user.email") {
            Ok(email) => Ok(Some(email)),
            Err(_) => Ok(None),
        }
    }

    /// Get the user's name from git config.
    pub fn config_user_name(&self) -> Result<Option<String>> {
        let config = self.repo.config()?;
        match config.get_string("user.name") {
            Ok(name) => Ok(Some(name)),
            Err(_) => Ok(None),
        }
    }

    /// Check if the working directory is clean (no uncommitted changes).
    pub fn is_clean(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None)?;
        Ok(statuses.is_empty())
    }

    /// Get the list of staged files.
    pub fn staged_files(&self) -> Result<Vec<String>> {
        let statuses = self.repo.statuses(None)?;
        let mut files = Vec::new();

        for entry in statuses.iter() {
            let status = entry.status();
            if status.is_index_new()
                || status.is_index_modified()
                || status.is_index_deleted()
                || status.is_index_renamed()
            {
                if let Some(path) = entry.path() {
                    files.push(path.to_string());
                }
            }
        }

        Ok(files)
    }

    /// Check if there are staged changes ready to commit.
    pub fn has_staged_changes(&self) -> Result<bool> {
        Ok(!self.staged_files()?.is_empty())
    }

    /// Create a git commit with the staged changes.
    ///
    /// # Arguments
    ///
    /// * `message` - The commit message
    ///
    /// # Returns
    ///
    /// The hash of the newly created commit.
    pub fn commit(&self, message: &str) -> Result<String> {
        // Get the signature from git config
        let sig = self.repo.signature()?;

        // Get the current index
        let mut index = self.repo.index()?;

        // Write the index as a tree
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        // Get the parent commit (HEAD)
        let parent = self.repo.head()?.peel_to_commit()?;

        // Create the commit
        let commit_id = self.repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            message,
            &tree,
            &[&parent],
        )?;

        Ok(commit_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, GitRepository) {
        let temp = TempDir::new().unwrap();

        // Initialize a git repository
        Repository::init(temp.path()).unwrap();

        // Create initial commit
        let repo = Repository::open(temp.path()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();

        // Create a file
        fs::write(temp.path().join("README.md"), "# Test").unwrap();

        // Stage and commit
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        let git_repo = GitRepository::open(temp.path()).unwrap();
        (temp, git_repo)
    }

    #[test]
    fn test_current_branch() {
        let (_temp, repo) = create_test_repo();
        let branch = repo.current_branch().unwrap();
        // Git 2.28+ defaults to "main", older versions use "master"
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn test_head_commit_hash() {
        let (_temp, repo) = create_test_repo();
        let hash = repo.head_commit_hash().unwrap();
        assert_eq!(hash.len(), 40); // SHA-1 hash
    }

    #[test]
    fn test_is_clean() {
        let (temp, repo) = create_test_repo();
        assert!(repo.is_clean().unwrap());

        // Create an untracked file
        fs::write(temp.path().join("new.txt"), "content").unwrap();
        assert!(!repo.is_clean().unwrap());
    }

    #[test]
    fn test_commit() {
        let (temp, git_repo) = create_test_repo();

        // Create and stage a new file
        fs::write(temp.path().join("new_file.txt"), "new content").unwrap();

        let repo = Repository::open(temp.path()).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("new_file.txt")).unwrap();
        index.write().unwrap();

        // Verify we have staged changes
        assert!(git_repo.has_staged_changes().unwrap());

        // Create a commit
        let hash = git_repo.commit("Test commit message").unwrap();
        assert_eq!(hash.len(), 40); // SHA-1 hash

        // Verify the commit is now HEAD
        assert_eq!(git_repo.head_commit_hash().unwrap(), hash);

        // Verify no more staged changes
        assert!(!git_repo.has_staged_changes().unwrap());
    }
}
