//! Commit pipeline implementation.
//!
//! Orchestrates the full commit workflow:
//! 1. Acquire lock
//!    1.5. Check for semantic conflicts (Safety Valve)
//! 2. Read index entries
//! 3. Create trace blob
//! 4. Create/get roadmap blob
//! 5. Create git commit (if staged changes exist)
//! 6. Get git commit hash
//! 7. Create neural commit
//! 8. Update refs
//! 9. Clear index
//! 10. Release lock (automatic on drop)

use std::path::PathBuf;

use crate::core::reconcile;
use crate::core::SynthesizeSummary;
use crate::domain::{BlobContent, NeuralCommit, WrappedBlob, WrappedNeuralCommit};
use crate::error::{AgitError, Result};
use crate::git::GitRepository;
use crate::safety::{lock_path, LockGuard};
use crate::storage::{
    FileHeadStore, FileIndexStore, FileObjectStore, FileRefStore, GitObjectStore, GitRefStore,
    HeadStore, IndexStore, ObjectStore, RefStore,
};

/// The state of changes detected in the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeState {
    /// Code + Memory changed - Standard git commit.
    CodeAndMemory,
    /// Memory only changed - \[Agit\] prefix commit.
    MemoryOnly,
    /// Nothing changed - Abort.
    NoChanges,
}

/// Result of a successful commit.
pub struct CommitResult {
    /// The hash of the new neural commit.
    pub neural_hash: String,
    /// The hash of the git commit.
    pub git_hash: String,
    /// Whether a new git commit was created (vs linking to existing HEAD).
    pub git_commit_created: bool,
    /// Whether this was a memory-only commit (with \[Agit\] prefix).
    pub is_memory_only: bool,
}

/// The commit pipeline orchestrates the full commit workflow.
pub struct CommitPipeline {
    agit_dir: PathBuf,
    git: GitRepository,
    objects: FileObjectStore,
    refs: FileRefStore,
    head: FileHeadStore,
    index: FileIndexStore,
}

impl CommitPipeline {
    /// Create a new commit pipeline with file-based storage (V1).
    pub fn new(
        agit_dir: PathBuf,
        git: GitRepository,
        objects: FileObjectStore,
        refs: FileRefStore,
        head: FileHeadStore,
        index: FileIndexStore,
    ) -> Self {
        Self {
            agit_dir,
            git,
            objects,
            refs,
            head,
            index,
        }
    }

    /// Detect the current change state.
    pub fn detect_change_state(&self) -> Result<ChangeState> {
        let has_staged = self.git.has_staged_changes()?;
        let has_code = self.git.has_code_changes()?;
        let has_index = !self.index.is_empty()?;
        let has_agit_only = self.git.has_agit_only_changes()?;

        if has_staged || has_code {
            Ok(ChangeState::CodeAndMemory)
        } else if has_index || has_agit_only {
            Ok(ChangeState::MemoryOnly)
        } else {
            Ok(ChangeState::NoChanges)
        }
    }

    /// Execute the commit pipeline.
    ///
    /// # Arguments
    ///
    /// * `message` - The commit message
    /// * `summary` - The synthesized summary
    /// * `force` - If true, skip semantic conflict check
    pub fn execute(&mut self, message: &str, summary: &str, force: bool) -> Result<CommitResult> {
        // 1. Acquire exclusive lock
        let _lock = LockGuard::acquire(&lock_path(&self.agit_dir))?;

        // 2. Read index entries
        let entries = if self.index.has_staged()? {
            self.index.read_staged()?
        } else {
            self.index.read_all()?
        };

        // 1.5. Check for semantic conflicts (Safety Valve)
        if !force {
            let branch = self.head.get()?.unwrap_or_else(|| "main".to_string());
            let conflict = reconcile::check_for_conflicts(
                &self.git,
                &self.objects,
                &self.refs,
                &branch,
                &entries,
            )?;

            if conflict.has_conflict {
                return Err(AgitError::SemanticConflict {
                    files: conflict.conflicting_files,
                });
            }
        }

        // 3. Create trace blob
        let trace_content = SynthesizeSummary::format_trace(&entries);
        let trace_blob = BlobContent::trace(&trace_content);
        let trace_json = serde_json::to_vec(&WrappedBlob::wrap(trace_blob))?;
        let trace_hash = self.objects.save(&trace_json)?;

        // 4. Get or create roadmap blob
        let roadmap_hash = self.get_or_create_roadmap()?;

        // 5. Get current branch
        let branch = self.head.get()?.unwrap_or_else(|| "main".to_string());

        // 6. Get parent neural commit hash(es)
        let parent_hashes = self.get_parent_hashes(&branch)?;

        // 7. Handle change state
        let change_state = self.detect_change_state()?;
        let (git_hash, git_commit_created, is_memory_only) = match change_state {
            ChangeState::CodeAndMemory => {
                if self.git.has_staged_changes()? {
                    (self.git.commit(message)?, true, false)
                } else {
                    (self.git.head_commit_hash()?, false, false)
                }
            },
            ChangeState::MemoryOnly => {
                // V1: Stage .agit/ and create git commit with [Agit] prefix
                self.git.stage_files(&[".agit/"])?;
                let prefixed = format!("[Agit] Context Update: {}", message);
                (self.git.commit(&prefixed)?, true, true)
            },
            ChangeState::NoChanges => {
                return Err(AgitError::NothingToCommit);
            },
        };

        // 8. Create neural commit
        let author = self
            .git
            .config_user_email()?
            .unwrap_or_else(|| "unknown".to_string());

        let neural_commit = if parent_hashes.len() > 1 {
            NeuralCommit::new_with_parents(
                &git_hash,
                parent_hashes,
                &author,
                &roadmap_hash,
                &trace_hash,
                summary,
            )
        } else {
            NeuralCommit::new(
                &git_hash,
                parent_hashes.into_iter().next(),
                &author,
                &roadmap_hash,
                &trace_hash,
                summary,
            )
        };

        // 9. Save neural commit
        let wrapped = WrappedNeuralCommit::wrap(neural_commit);
        let commit_json = serde_json::to_vec(&wrapped)?;
        let neural_hash = self.objects.save(&commit_json)?;

        // 10. Update branch ref
        self.refs.update(&branch, &neural_hash)?;

        // 10.5. Index entries for full-text search (non-fatal)
        if let Err(e) = crate::search::indexer::index_entries(&self.agit_dir, &entries) {
            tracing::warn!("Failed to index entries for search: {}", e);
        }

        // 11. Clear index
        if self.index.has_staged()? {
            self.index.clear_staged()?;
        } else {
            self.index.clear()?;
        }

        Ok(CommitResult {
            neural_hash,
            git_hash,
            git_commit_created,
            is_memory_only,
        })
    }

    /// Get or create the roadmap blob.
    fn get_or_create_roadmap(&self) -> Result<String> {
        let roadmap =
            BlobContent::roadmap("No roadmap set. Use 'agit roadmap' to set project goals.");
        let wrapped = WrappedBlob::wrap(roadmap);
        let json = serde_json::to_vec(&wrapped)?;
        self.objects.save(&json)
    }

    /// Find neural commit hash by git commit hash.
    fn find_neural_by_git_hash(&self, git_hash: &str) -> Result<Option<String>> {
        for branch in self.refs.list()? {
            if let Some(mut neural_hash) = self.refs.get(&branch)? {
                let mut visited = std::collections::HashSet::new();
                loop {
                    if visited.contains(&neural_hash) {
                        break;
                    }
                    visited.insert(neural_hash.clone());

                    let data = self.objects.load(&neural_hash)?;
                    let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;

                    if wrapped.data.git_hash.starts_with(git_hash)
                        || git_hash.starts_with(&wrapped.data.git_hash)
                    {
                        return Ok(Some(neural_hash));
                    }

                    if let Some(parent) = wrapped.data.first_parent() {
                        neural_hash = parent.to_string();
                    } else {
                        break;
                    }
                }
            }
        }
        Ok(None)
    }

    /// Get parent hashes for the neural commit.
    fn get_parent_hashes(&self, branch: &str) -> Result<Vec<String>> {
        let mut parents = Vec::new();

        if let Some(hash) = self.refs.get(branch)? {
            parents.push(hash);
        }

        if self.git.is_merging()? {
            if let Some(merge_git_hash) = self.git.merge_head_hash()? {
                if let Some(neural_hash) = self.find_neural_by_git_hash(&merge_git_hash)? {
                    if !parents.contains(&neural_hash) {
                        parents.push(neural_hash);
                    }
                }
            }
        }

        Ok(parents)
    }
}

/// Git-native commit pipeline using Git ODB and refs/agit/* namespace.
///
/// This is the V2 storage implementation that makes Agit invisible
/// in `git status` and `git branch -a`.
pub struct GitNativeCommitPipeline {
    agit_dir: PathBuf,
    git: GitRepository,
    objects: GitObjectStore,
    refs: GitRefStore,
    head: FileHeadStore,
    index: FileIndexStore,
}

impl GitNativeCommitPipeline {
    /// Create a new Git-native commit pipeline (V2).
    ///
    /// # Arguments
    ///
    /// * `agit_dir` - Path to the .agit directory (for local state)
    /// * `git` - Git repository wrapper
    pub fn new(agit_dir: PathBuf, git: GitRepository) -> Result<Self> {
        let repo_path = git
            .workdir()
            .ok_or(AgitError::NotGitRepository)?
            .to_path_buf();

        Ok(Self {
            agit_dir: agit_dir.clone(),
            git,
            objects: GitObjectStore::new(&repo_path),
            refs: GitRefStore::new(&repo_path),
            head: FileHeadStore::new(&agit_dir),
            index: FileIndexStore::new(&agit_dir),
        })
    }

    /// Detect the current change state.
    pub fn detect_change_state(&self) -> Result<ChangeState> {
        let has_staged = self.git.has_staged_changes()?;
        let has_code = self.git.has_code_changes()?;
        let has_index = !self.index.is_empty()?;

        if has_staged || has_code {
            Ok(ChangeState::CodeAndMemory)
        } else if has_index {
            // In V2, memory-only doesn't stage .agit/ - just creates neural commit
            Ok(ChangeState::MemoryOnly)
        } else {
            Ok(ChangeState::NoChanges)
        }
    }

    /// Execute the commit pipeline.
    ///
    /// For V2 Git-native storage:
    /// - Code changes: Create Git commit, then neural commit pointing to it
    /// - Memory-only: Create neural commit only (no Git commit needed)
    ///
    /// # Arguments
    ///
    /// * `message` - The commit message
    /// * `summary` - The synthesized summary
    /// * `force` - If true, skip semantic conflict check
    pub fn execute(&mut self, message: &str, summary: &str, force: bool) -> Result<CommitResult> {
        // 1. Acquire exclusive lock
        let _lock = LockGuard::acquire(&lock_path(&self.agit_dir))?;

        // 2. Read index entries
        let entries = if self.index.has_staged()? {
            self.index.read_staged()?
        } else {
            self.index.read_all()?
        };

        // 1.5. Check for semantic conflicts (Safety Valve)
        if !force {
            let branch = self.head.get()?.unwrap_or_else(|| "main".to_string());
            let conflict = reconcile::check_for_conflicts(
                &self.git,
                &self.objects,
                &self.refs,
                &branch,
                &entries,
            )?;

            if conflict.has_conflict {
                return Err(AgitError::SemanticConflict {
                    files: conflict.conflicting_files,
                });
            }
        }

        // 3. Create trace blob
        let trace_content = SynthesizeSummary::format_trace(&entries);
        let trace_blob = BlobContent::trace(&trace_content);
        let trace_json = serde_json::to_vec(&WrappedBlob::wrap(trace_blob))?;
        let trace_hash = self.objects.save(&trace_json)?;

        // 4. Get or create roadmap blob
        let roadmap_hash = self.get_or_create_roadmap()?;

        // 5. Get current branch
        let branch = self.head.get()?.unwrap_or_else(|| "main".to_string());

        // 6. Get parent neural commit hash(es)
        let parent_hashes = self.get_parent_hashes(&branch)?;

        // 7. Handle change state
        let change_state = self.detect_change_state()?;
        let (git_hash, git_commit_created, is_memory_only) = match change_state {
            ChangeState::CodeAndMemory => {
                if self.git.has_staged_changes()? {
                    (self.git.commit(message)?, true, false)
                } else {
                    (self.git.head_commit_hash()?, false, false)
                }
            },
            ChangeState::MemoryOnly => {
                // V2: No Git commit for memory-only - just link to current HEAD
                (self.git.head_commit_hash()?, false, true)
            },
            ChangeState::NoChanges => {
                return Err(AgitError::NothingToCommit);
            },
        };

        // 8. Create neural commit
        let author = self
            .git
            .config_user_email()?
            .unwrap_or_else(|| "unknown".to_string());

        let neural_commit = if parent_hashes.len() > 1 {
            NeuralCommit::new_with_parents(
                &git_hash,
                parent_hashes,
                &author,
                &roadmap_hash,
                &trace_hash,
                summary,
            )
        } else {
            NeuralCommit::new(
                &git_hash,
                parent_hashes.into_iter().next(),
                &author,
                &roadmap_hash,
                &trace_hash,
                summary,
            )
        };

        // 9. Save neural commit
        let wrapped = WrappedNeuralCommit::wrap(neural_commit);
        let commit_json = serde_json::to_vec(&wrapped)?;
        let neural_hash = self.objects.save(&commit_json)?;

        // 10. Update branch ref
        self.refs.update(&branch, &neural_hash)?;

        // 10.5. Index entries for full-text search (non-fatal)
        if let Err(e) = crate::search::indexer::index_entries(&self.agit_dir, &entries) {
            tracing::warn!("Failed to index entries for search: {}", e);
        }

        // 11. Clear index
        if self.index.has_staged()? {
            self.index.clear_staged()?;
        } else {
            self.index.clear()?;
        }

        Ok(CommitResult {
            neural_hash,
            git_hash,
            git_commit_created,
            is_memory_only,
        })
    }

    /// Get or create the roadmap blob.
    fn get_or_create_roadmap(&self) -> Result<String> {
        let roadmap =
            BlobContent::roadmap("No roadmap set. Use 'agit roadmap' to set project goals.");
        let wrapped = WrappedBlob::wrap(roadmap);
        let json = serde_json::to_vec(&wrapped)?;
        self.objects.save(&json)
    }

    /// Find neural commit hash by git commit hash.
    fn find_neural_by_git_hash(&self, git_hash: &str) -> Result<Option<String>> {
        for branch in self.refs.list()? {
            if let Some(mut neural_hash) = self.refs.get(&branch)? {
                let mut visited = std::collections::HashSet::new();
                loop {
                    if visited.contains(&neural_hash) {
                        break;
                    }
                    visited.insert(neural_hash.clone());

                    let data = self.objects.load(&neural_hash)?;
                    let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;

                    if wrapped.data.git_hash.starts_with(git_hash)
                        || git_hash.starts_with(&wrapped.data.git_hash)
                    {
                        return Ok(Some(neural_hash));
                    }

                    if let Some(parent) = wrapped.data.first_parent() {
                        neural_hash = parent.to_string();
                    } else {
                        break;
                    }
                }
            }
        }
        Ok(None)
    }

    /// Get parent hashes for the neural commit.
    fn get_parent_hashes(&self, branch: &str) -> Result<Vec<String>> {
        let mut parents = Vec::new();

        if let Some(hash) = self.refs.get(branch)? {
            parents.push(hash);
        }

        if self.git.is_merging()? {
            if let Some(merge_git_hash) = self.git.merge_head_hash()? {
                if let Some(neural_hash) = self.find_neural_by_git_hash(&merge_git_hash)? {
                    if !parents.contains(&neural_hash) {
                        parents.push(neural_hash);
                    }
                }
            }
        }

        Ok(parents)
    }
}
