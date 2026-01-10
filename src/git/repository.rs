//! Git repository wrapper.

use std::path::Path;

use git2::Repository;

use crate::error::Result;

/// Metadata for a Git commit, used for amend detection.
#[derive(Debug, Clone)]
pub struct CommitMetadata {
    /// Author's email address.
    pub author_email: String,
    /// First line of the commit message.
    pub message_first_line: String,
    /// Unix timestamp of when the commit was authored.
    pub timestamp: i64,
}

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

    /// Get the path to the repository working directory.
    ///
    /// This is the root directory containing `.git/`.
    pub fn workdir(&self) -> Option<&Path> {
        self.repo.workdir()
    }

    /// Get the path to the `.git` directory.
    pub fn git_dir(&self) -> &Path {
        self.repo.path()
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
        let commit_id = self
            .repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?;

        Ok(commit_id.to_string())
    }

    /// Create an empty git commit (same tree as parent).
    ///
    /// Used for Journal Entries when no code changes are present.
    /// This is equivalent to `git commit --allow-empty`.
    ///
    /// # Arguments
    ///
    /// * `message` - The commit message
    ///
    /// # Returns
    ///
    /// The hash of the newly created (empty) commit.
    pub fn commit_empty(&self, message: &str) -> Result<String> {
        let sig = self.repo.signature()?;

        // Get the parent commit and its tree
        let parent = self.repo.head()?.peel_to_commit()?;
        let tree = parent.tree()?;

        // Create commit with same tree as parent (empty commit)
        let commit_id = self
            .repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?;

        Ok(commit_id.to_string())
    }

    /// Stage files matching the given pathspecs.
    pub fn stage_files(&self, pathspecs: &[&str]) -> Result<usize> {
        let mut index = self.repo.index()?;
        let mut count = 0;

        index.add_all(
            pathspecs.iter(),
            git2::IndexAddOption::DEFAULT,
            Some(&mut |path, _| {
                count += 1;
                println!("  add: {}", path.to_string_lossy());
                0
            }),
        )?;

        // Handle deleted files
        index.update_all(pathspecs.iter(), None)?;

        index.write()?;
        Ok(count)
    }

    /// Check if there are changes outside the .agit/ directory.
    ///
    /// This is used to detect "code changes" vs "memory-only changes".
    pub fn has_code_changes(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None)?;

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                // Check if path is NOT under .agit/
                if !path.starts_with(".agit/") && !path.starts_with(".agit\\") {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Check if there are ONLY .agit/ directory changes.
    ///
    /// Returns true if there are changes and ALL of them are under .agit/.
    /// Returns false if there are no changes or if any changes are outside .agit/.
    pub fn has_agit_only_changes(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None)?;
        let mut has_agit_changes = false;

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                if path.starts_with(".agit/") || path.starts_with(".agit\\") {
                    has_agit_changes = true;
                } else {
                    // Found a non-.agit change
                    return Ok(false);
                }
            }
        }

        Ok(has_agit_changes)
    }

    /// Check if we're currently in a merge state.
    ///
    /// This is detected by the presence of .git/MERGE_HEAD file.
    pub fn is_merging(&self) -> Result<bool> {
        let merge_head_path = self.repo.path().join("MERGE_HEAD");
        Ok(merge_head_path.exists())
    }

    /// Check if we're currently in a rebase state.
    ///
    /// This is detected by the presence of .git/rebase-merge/ or .git/rebase-apply/ directories.
    pub fn is_rebasing(&self) -> Result<bool> {
        let rebase_merge_path = self.repo.path().join("rebase-merge");
        let rebase_apply_path = self.repo.path().join("rebase-apply");
        Ok(rebase_merge_path.exists() || rebase_apply_path.exists())
    }

    /// Check if we're in any conflicted state (merge or rebase in progress).
    ///
    /// When in a conflicted state, Agit commands that modify the graph should be blocked.
    pub fn is_in_conflicted_state(&self) -> Result<bool> {
        Ok(self.is_merging()? || self.is_rebasing()?)
    }

    /// Get the MERGE_HEAD hash if in merge state.
    ///
    /// Returns None if not in merge state.
    pub fn merge_head_hash(&self) -> Result<Option<String>> {
        let merge_head_path = self.repo.path().join("MERGE_HEAD");

        if merge_head_path.exists() {
            let content = std::fs::read_to_string(&merge_head_path)?;
            Ok(Some(content.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    /// Get metadata for a specific commit.
    ///
    /// This is used for amend detection - comparing commit properties
    /// to detect if a commit is a rewritten version of another.
    ///
    /// # Arguments
    ///
    /// * `hash` - The commit hash to get metadata for
    ///
    /// # Returns
    ///
    /// `CommitMetadata` containing author email, first line of message, and timestamp.
    pub fn get_commit_metadata(&self, hash: &str) -> Result<CommitMetadata> {
        let oid = git2::Oid::from_str(hash)?;
        let commit = self.repo.find_commit(oid)?;

        let author = commit.author();
        let author_email = author.email().unwrap_or("unknown@unknown.com").to_string();

        let message = commit.message().unwrap_or("");
        let message_first_line = message.lines().next().unwrap_or("").to_string();

        let timestamp = author.when().seconds();

        Ok(CommitMetadata {
            author_email,
            message_first_line,
            timestamp,
        })
    }

    /// Get the list of files changed between two commits.
    ///
    /// This computes the diff between the trees of two commits and returns
    /// the paths of all files that were added, modified, or deleted.
    ///
    /// # Arguments
    ///
    /// * `from_hash` - The starting commit hash (older commit)
    /// * `to_hash` - The ending commit hash (newer commit)
    ///
    /// # Returns
    ///
    /// A vector of file paths that changed between the two commits.
    pub fn diff_commits(&self, from_hash: &str, to_hash: &str) -> Result<Vec<String>> {
        let from_oid = git2::Oid::from_str(from_hash)?;
        let to_oid = git2::Oid::from_str(to_hash)?;

        let from_commit = self.repo.find_commit(from_oid)?;
        let to_commit = self.repo.find_commit(to_oid)?;

        let from_tree = from_commit.tree()?;
        let to_tree = to_commit.tree()?;

        let diff = self
            .repo
            .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), None)?;

        let mut changed_files = Vec::new();

        for delta in diff.deltas() {
            // Get the new file path (or old path for deletions)
            if let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path()) {
                if let Some(path_str) = path.to_str() {
                    changed_files.push(path_str.to_string());
                }
            }
        }

        Ok(changed_files)
    }

    /// Check if `ancestor` is reachable from `descendant` (i.e., ancestor is in history).
    ///
    /// This is used to detect "dangling head" scenarios where Agit points to a
    /// commit that no longer exists in Git's history (e.g., after `git reset --hard`).
    ///
    /// # Arguments
    ///
    /// * `ancestor` - The commit hash to check if it's an ancestor
    /// * `descendant` - The commit hash to check if ancestor is reachable from
    ///
    /// # Returns
    ///
    /// `true` if ancestor is reachable from descendant, `false` otherwise.
    pub fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool> {
        let ancestor_oid = git2::Oid::from_str(ancestor)?;
        let descendant_oid = git2::Oid::from_str(descendant)?;

        // graph_descendant_of returns true if `descendant` is a descendant of `ancestor`
        Ok(self
            .repo
            .graph_descendant_of(descendant_oid, ancestor_oid)?)
    }

    /// Get the list of commits between two commit hashes (exclusive of from, inclusive of to).
    ///
    /// This walks the commit history from `to_hash` back to `from_hash` and returns
    /// all commit hashes in between (not including `from_hash`).
    ///
    /// # Arguments
    ///
    /// * `from_hash` - The older commit (exclusive - not included in result)
    /// * `to_hash` - The newer commit (inclusive - included in result)
    ///
    /// # Returns
    ///
    /// A vector of commit hashes from newest to oldest.
    pub fn commits_between(&self, from_hash: &str, to_hash: &str) -> Result<Vec<String>> {
        let from_oid = git2::Oid::from_str(from_hash)?;
        let to_oid = git2::Oid::from_str(to_hash)?;

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push(to_oid)?;
        revwalk.hide(from_oid)?;

        let mut commits = Vec::new();
        for oid_result in revwalk {
            let oid = oid_result?;
            commits.push(oid.to_string());
        }

        Ok(commits)
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

        // Configure git user for tests (required in CI environments)
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
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
