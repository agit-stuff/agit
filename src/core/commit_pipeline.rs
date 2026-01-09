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
use crate::error::Result;
use crate::git::GitRepository;
use crate::safety::{lock_path, LockGuard};
use crate::storage::{
    FileHeadStore, FileIndexStore, FileObjectStore, FileRefStore, HeadStore, IndexStore,
    ObjectStore, RefStore,
};

/// Result of a successful commit.
pub struct CommitResult {
    /// The hash of the new neural commit.
    pub neural_hash: String,
    /// The hash of the git commit.
    pub git_hash: String,
    /// Whether a new git commit was created (vs linking to existing HEAD).
    pub git_commit_created: bool,
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

    /// Execute the commit pipeline.
    ///
    /// # Arguments
    ///
    /// * `message` - The git commit message
    /// * `summary` - The synthesized summary for the neural commit
    pub fn execute(&mut self, message: &str, summary: &str) -> Result<CommitResult> {
        // 1. Acquire exclusive lock
        let _lock = LockGuard::acquire(&lock_path(&self.agit_dir))?;

        // 2. Read index entries
        let entries = self.index.read_all()?;

        // 2. Create trace blob
        let trace_content = SynthesizeSummary::format_trace(&entries);
        let trace_blob = BlobContent::trace(&trace_content);
        let trace_json = serde_json::to_vec(&WrappedBlob::wrap(trace_blob))?;
        let trace_hash = self.objects.save(&trace_json)?;

        // 3. Get or create roadmap blob (for now, create empty if none)
        let roadmap_hash = self.get_or_create_roadmap()?;

        // 4. Get current branch
        let branch = self.head.get()?.unwrap_or_else(|| "main".to_string());

        // 5. Get parent neural commit hash
        let parent_hash = self.refs.get(&branch)?;

        // 6. Create git commit if there are staged changes
        let (git_hash, git_commit_created) = if self.git.has_staged_changes()? {
            (self.git.commit(message)?, true)
        } else {
            // No staged changes - link to current HEAD
            (self.git.head_commit_hash()?, false)
        };

        // 7. Create neural commit
        let author = self
            .git
            .config_user_email()?
            .unwrap_or_else(|| "unknown".to_string());
        let neural_commit = NeuralCommit::new(
            &git_hash,
            parent_hash,
            &author,
            &roadmap_hash,
            &trace_hash,
            summary,
        );

        // 8. Save neural commit
        let wrapped = WrappedNeuralCommit::wrap(neural_commit);
        let commit_json = serde_json::to_vec(&wrapped)?;
        let neural_hash = self.objects.save(&commit_json)?;

        // 9. Update branch ref
        self.refs.update(&branch, &neural_hash)?;

        // 10. Clear index
        self.index.clear()?;

        Ok(CommitResult {
            neural_hash,
            git_hash,
            git_commit_created,
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
}
