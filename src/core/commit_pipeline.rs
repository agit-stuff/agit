//! Commit pipeline implementation.
//!
//! Orchestrates the full commit workflow:
//! 1. Acquire lock
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

use crate::core::SynthesizeSummary;
use crate::domain::{BlobContent, NeuralCommit, WrappedBlob, WrappedNeuralCommit};
use crate::error::{AgitError, Result};
use crate::git::GitRepository;
use crate::safety::{lock_path, LockGuard};
use crate::storage::{
    FileHeadStore, FileIndexStore, FileObjectStore, FileRefStore, HeadStore, IndexStore,
    ObjectStore, RefStore,
};

/// The state of changes detected in the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeState {
    /// Code + Memory changed - Standard git commit.
    CodeAndMemory,
    /// Memory only changed - [Agit] prefix commit.
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
    /// Whether this was a memory-only commit (with [Agit] prefix).
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
    /// Create a new commit pipeline.
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
    ///
    /// Determines whether we have:
    /// - Code + memory changes (standard commit)
    /// - Memory-only changes ([Agit] prefix commit)
    /// - No changes (abort)
    pub fn detect_change_state(&self) -> Result<ChangeState> {
        let has_staged = self.git.has_staged_changes()?;
        let has_code = self.git.has_code_changes()?;
        let has_index = !self.index.is_empty()?;
        let has_agit_only = self.git.has_agit_only_changes()?;

        if has_staged || has_code {
            // Code changes present - standard commit
            Ok(ChangeState::CodeAndMemory)
        } else if has_index || has_agit_only {
            // Only memory changes - [Agit] prefix commit
            Ok(ChangeState::MemoryOnly)
        } else {
            // Nothing to commit
            Ok(ChangeState::NoChanges)
        }
    }

    /// Execute the commit pipeline.
    ///
    /// # Arguments
    ///
    /// * `message` - The git commit message
    /// * `summary` - The synthesized summary for the neural commit
    pub fn execute(&mut self, message: &str, summary: &str) -> Result<CommitResult> {
        // 1. Acquire exclusive lock
        let _lock = LockGuard::acquire(&lock_path(&self.agit_dir))?;

        // 2. Read index entries (from staged-index if exists, otherwise from index)
        let entries = if self.index.has_staged()? {
            self.index.read_staged()?
        } else {
            self.index.read_all()?
        };

        // 2. Create trace blob
        let trace_content = SynthesizeSummary::format_trace(&entries);
        let trace_blob = BlobContent::trace(&trace_content);
        let trace_json = serde_json::to_vec(&WrappedBlob::wrap(trace_blob))?;
        let trace_hash = self.objects.save(&trace_json)?;

        // 3. Get or create roadmap blob (for now, create empty if none)
        let roadmap_hash = self.get_or_create_roadmap()?;

        // 4. Get current branch
        let branch = self.head.get()?.unwrap_or_else(|| "main".to_string());

        // 5. Get parent neural commit hash(es) - handles merge state
        let parent_hashes = self.get_parent_hashes(&branch)?;

        // 6. Detect change state and create git commit accordingly
        let change_state = self.detect_change_state()?;
        let (final_message, git_hash, git_commit_created, is_memory_only) = match change_state {
            ChangeState::CodeAndMemory => {
                // Standard commit - use message as-is
                if self.git.has_staged_changes()? {
                    (message.to_string(), self.git.commit(message)?, true, false)
                } else {
                    // Code changes but not staged - link to HEAD
                    (message.to_string(), self.git.head_commit_hash()?, false, false)
                }
            }
            ChangeState::MemoryOnly => {
                // Memory-only commit - stage .agit/ and prefix message
                self.git.stage_files(&[".agit/"])?;
                let prefixed = format!("[Agit] Context Update: {}", message);
                (prefixed.clone(), self.git.commit(&prefixed)?, true, true)
            }
            ChangeState::NoChanges => {
                // Nothing to commit - return error
                return Err(AgitError::NothingToCommit);
            }
        };

        // Use final_message for any logging if needed
        let _ = final_message;

        // 7. Create neural commit
        let author = self
            .git
            .config_user_email()?
            .unwrap_or_else(|| "unknown".to_string());

        // Use multi-parent constructor if we have multiple parents (merge commit)
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
            // Single parent or root commit - use standard constructor
            NeuralCommit::new(
                &git_hash,
                parent_hashes.into_iter().next(),
                &author,
                &roadmap_hash,
                &trace_hash,
                summary,
            )
        };

        // 8. Save neural commit
        let wrapped = WrappedNeuralCommit::wrap(neural_commit);
        let commit_json = serde_json::to_vec(&wrapped)?;
        let neural_hash = self.objects.save(&commit_json)?;

        // 9. Update branch ref
        self.refs.update(&branch, &neural_hash)?;

        // 10. Clear index (staged-index if exists, otherwise regular index)
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
        // For now, just create an empty roadmap
        // In the future, this should read from .agit/roadmap or similar
        let roadmap =
            BlobContent::roadmap("No roadmap set. Use 'agit roadmap' to set project goals.");
        let wrapped = WrappedBlob::wrap(roadmap);
        let json = serde_json::to_vec(&wrapped)?;
        self.objects.save(&json)
    }

    /// Find neural commit hash by git commit hash.
    ///
    /// Walks the neural graph from all refs to find a neural commit
    /// that links to the given git hash. Returns None if not found.
    fn find_neural_by_git_hash(&self, git_hash: &str) -> Result<Option<String>> {
        // Walk through all branch refs
        for branch in self.refs.list()? {
            if let Some(mut neural_hash) = self.refs.get(&branch)? {
                // Walk the commit chain
                let mut visited = std::collections::HashSet::new();
                loop {
                    if visited.contains(&neural_hash) {
                        break; // Cycle detection
                    }
                    visited.insert(neural_hash.clone());

                    let data = self.objects.load(&neural_hash)?;
                    let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;

                    // Check if git hash matches (prefix match supported)
                    if wrapped.data.git_hash.starts_with(git_hash)
                        || git_hash.starts_with(&wrapped.data.git_hash)
                    {
                        return Ok(Some(neural_hash));
                    }

                    // Move to first parent
                    if let Some(parent) = wrapped.data.first_parent() {
                        neural_hash = parent.to_string();
                    } else {
                        break; // Root commit
                    }
                }
            }
        }
        Ok(None)
    }

    /// Get parent hashes for the neural commit.
    ///
    /// Handles merge state by collecting parents from both branches.
    fn get_parent_hashes(&self, branch: &str) -> Result<Vec<String>> {
        let mut parents = Vec::new();

        // Current branch's head
        if let Some(hash) = self.refs.get(branch)? {
            parents.push(hash);
        }

        // Check if we're in a merge state
        if self.git.is_merging()? {
            // Get the merged branch's neural commit
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
