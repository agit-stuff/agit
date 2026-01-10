//! Branch synchronization between Git and AGIT.
//!
//! This module ensures the AGIT HEAD tracks the Git branch, keeping
//! the neural graph aligned with the code history.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::git::GitRepository;
use crate::storage::{FileHeadStore, FileIndexStore, FileRefStore, HeadStore, RefStore};

/// Branch synchronization status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    /// AGIT and Git are on the same branch.
    InSync { branch: String },
    /// AGIT is on a different branch than Git.
    OutOfSync {
        git_branch: String,
        agit_branch: String,
    },
    /// AGIT has no HEAD set (fresh initialization).
    NoAgitHead { git_branch: String },
}

impl SyncStatus {
    /// Check if branches are in sync.
    pub fn is_in_sync(&self) -> bool {
        matches!(self, SyncStatus::InSync { .. })
    }
}

/// Branch synchronizer that keeps AGIT aligned with Git.
pub struct BranchSync {
    git: GitRepository,
    head_store: FileHeadStore,
    ref_store: FileRefStore,
    index_store: FileIndexStore,
    #[allow(dead_code)]
    agit_dir: PathBuf,
}

impl BranchSync {
    /// Create a new branch synchronizer.
    pub fn new(project_root: &Path, agit_dir: &Path) -> Result<Self> {
        let git = GitRepository::open(project_root)?;
        let head_store = FileHeadStore::new(agit_dir);
        let ref_store = FileRefStore::new(agit_dir);
        let index_store = FileIndexStore::new(agit_dir);

        Ok(Self {
            git,
            head_store,
            ref_store,
            index_store,
            agit_dir: agit_dir.to_path_buf(),
        })
    }

    /// Get the current synchronization status.
    pub fn status(&self) -> Result<SyncStatus> {
        let git_branch = self.git.current_branch()?;
        let agit_branch = self.head_store.get()?;

        match agit_branch {
            Some(agit) if agit == git_branch => Ok(SyncStatus::InSync { branch: git_branch }),
            Some(agit) => Ok(SyncStatus::OutOfSync {
                git_branch,
                agit_branch: agit,
            }),
            None => Ok(SyncStatus::NoAgitHead { git_branch }),
        }
    }

    /// Synchronize AGIT HEAD to match Git's current branch.
    ///
    /// This updates AGIT's HEAD to point to the same branch as Git.
    /// If the branch doesn't exist in AGIT refs, it will be created
    /// as empty (no commits yet on that branch).
    pub fn sync(&self) -> Result<SyncResult> {
        let git_branch = self.git.current_branch()?;
        let old_branch = self.head_store.get()?;

        // Update AGIT HEAD to point to the Git branch
        self.head_store.set(&git_branch)?;

        // Check if the new branch has any commits
        let has_commits = self.ref_store.get(&git_branch)?.is_some();

        Ok(SyncResult {
            old_branch,
            new_branch: git_branch,
            has_commits,
        })
    }

    /// Check if we need to sync (i.e., branches are different).
    pub fn needs_sync(&self) -> Result<bool> {
        Ok(!self.status()?.is_in_sync())
    }

    /// Get the current Git branch.
    pub fn git_branch(&self) -> Result<String> {
        self.git.current_branch()
    }

    /// Get the current AGIT branch (from HEAD).
    pub fn agit_branch(&self) -> Result<Option<String>> {
        self.head_store.get()
    }

    /// Create a new branch in AGIT refs.
    ///
    /// This copies the current branch's HEAD to a new branch name.
    pub fn create_branch(&self, name: &str) -> Result<()> {
        let current = self.head_store.get()?.unwrap_or_else(|| "main".to_string());

        // Get the current commit hash (if any)
        if let Some(hash) = self.ref_store.get(&current)? {
            self.ref_store.update(name, &hash)?;
        }
        // If no commits yet, the new branch will also have no commits

        Ok(())
    }

    /// Switch AGIT to a different branch.
    ///
    /// Note: This only updates AGIT's HEAD, not Git's.
    /// Normally you should use `sync()` to follow Git.
    pub fn checkout(&self, branch: &str) -> Result<()> {
        self.head_store.set(branch)
    }

    /// Ensure AGIT is synced with the current Git branch.
    ///
    /// This is the main entry point for lazy sync. Call this at the start
    /// of every AGIT command to ensure the neural graph follows Git.
    ///
    /// When switching branches, this also handles per-branch index stashing:
    /// - Stashes the current index to `.agit/stash/<old_branch>/index`
    /// - Restores the index from `.agit/stash/<new_branch>/index` (or clears if none)
    ///
    /// - If already in sync: Returns `AlreadyInSync`
    /// - If Git branch exists in AGIT: Switches HEAD, returns `SwitchedToExisting`
    /// - If Git branch is new: Forks memory from current point, returns `ForkedToNew`
    pub fn ensure_branch_sync(&self) -> Result<EnsureSyncResult> {
        let git_branch = self.git.current_branch()?;
        let agit_branch = self.head_store.get()?;

        match agit_branch {
            Some(ref ab) if ab == &git_branch => {
                // Already in sync - no action needed
                Ok(EnsureSyncResult::AlreadyInSync { branch: git_branch })
            },
            Some(old_branch) => {
                // Git switched to a different branch

                // Stash the current index for the old branch (preserve pending thoughts)
                let stashed = self.index_store.stash_to_branch(&old_branch)?;
                if stashed {
                    eprintln!("🔀 Stashed pending thoughts for branch '{}'", old_branch);
                }

                if self.ref_store.get(&git_branch)?.is_some() {
                    // Branch exists in AGIT refs - just switch HEAD
                    self.head_store.set(&git_branch)?;

                    // Restore index from the new branch's stash (or clear if none)
                    let restored = self.index_store.restore_from_branch(&git_branch)?;
                    if restored {
                        eprintln!("🔀 Restored pending thoughts for branch '{}'", git_branch);
                    }

                    Ok(EnsureSyncResult::SwitchedToExisting {
                        old_branch,
                        new_branch: git_branch,
                    })
                } else {
                    // New branch - fork memory from current point
                    let fork_point = self.ref_store.get(&old_branch)?;
                    if let Some(ref hash) = fork_point {
                        // Copy current branch's head to new branch
                        self.ref_store.update(&git_branch, hash)?;
                    }
                    self.head_store.set(&git_branch)?;

                    // Try to restore index from the new branch's stash (or clear if none)
                    let restored = self.index_store.restore_from_branch(&git_branch)?;
                    if restored {
                        eprintln!("🔀 Restored pending thoughts for branch '{}'", git_branch);
                    }

                    Ok(EnsureSyncResult::ForkedToNew {
                        old_branch,
                        new_branch: git_branch,
                        fork_point,
                    })
                }
            },
            None => {
                // No AGIT HEAD set - fresh initialization
                self.head_store.set(&git_branch)?;
                Ok(EnsureSyncResult::ForkedToNew {
                    old_branch: String::new(),
                    new_branch: git_branch,
                    fork_point: None,
                })
            },
        }
    }
}

/// Result of a sync operation.
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// The previous AGIT branch (if any).
    pub old_branch: Option<String>,
    /// The new AGIT branch (matching Git).
    pub new_branch: String,
    /// Whether the new branch has any neural commits.
    pub has_commits: bool,
}

/// Result of an ensure_branch_sync operation.
#[derive(Debug, Clone)]
pub enum EnsureSyncResult {
    /// AGIT and Git are already in sync.
    AlreadyInSync { branch: String },
    /// Switched to an existing AGIT branch.
    SwitchedToExisting {
        old_branch: String,
        new_branch: String,
    },
    /// Forked memory to a new branch (Git branch didn't exist in AGIT).
    ForkedToNew {
        old_branch: String,
        new_branch: String,
        fork_point: Option<String>,
    },
}

impl SyncResult {
    /// Check if the branch actually changed.
    pub fn changed(&self) -> bool {
        self.old_branch.as_ref() != Some(&self.new_branch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, std::path::PathBuf) {
        let temp = TempDir::new().unwrap();

        // Initialize git repo
        let repo = Repository::init(temp.path()).unwrap();

        // Create initial commit
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        fs::write(temp.path().join("README.md"), "# Test").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        // Create .agit directory
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(agit_dir.join("refs/heads")).unwrap();
        fs::write(agit_dir.join("HEAD"), "main").unwrap();

        (temp, agit_dir)
    }

    #[test]
    fn test_sync_status_in_sync() {
        let (temp, agit_dir) = setup_test_env();

        // Get the actual git branch name
        let git_repo = GitRepository::open(temp.path()).unwrap();
        let git_branch = git_repo.current_branch().unwrap();

        // Update AGIT HEAD to match
        fs::write(agit_dir.join("HEAD"), &git_branch).unwrap();

        let sync = BranchSync::new(temp.path(), &agit_dir).unwrap();
        let status = sync.status().unwrap();

        assert!(status.is_in_sync());
    }

    #[test]
    fn test_sync_status_out_of_sync() {
        let (temp, agit_dir) = setup_test_env();

        // Set AGIT to a different branch
        fs::write(agit_dir.join("HEAD"), "feature-x").unwrap();

        let sync = BranchSync::new(temp.path(), &agit_dir).unwrap();
        let status = sync.status().unwrap();

        assert!(!status.is_in_sync());
        if let SyncStatus::OutOfSync { agit_branch, .. } = status {
            assert_eq!(agit_branch, "feature-x");
        } else {
            panic!("Expected OutOfSync status");
        }
    }

    #[test]
    fn test_sync_operation() {
        let (temp, agit_dir) = setup_test_env();

        // Set AGIT to a different branch
        fs::write(agit_dir.join("HEAD"), "old-branch").unwrap();

        let sync = BranchSync::new(temp.path(), &agit_dir).unwrap();

        // Verify out of sync
        assert!(sync.needs_sync().unwrap());

        // Perform sync
        let result = sync.sync().unwrap();

        assert_eq!(result.old_branch, Some("old-branch".to_string()));
        assert!(result.changed());

        // Verify now in sync
        assert!(!sync.needs_sync().unwrap());
    }
}
