//! Neural commit types.
//!
//! A neural commit links a Git commit to its associated context,
//! including the reasoning trace, roadmap, and synthesized summary.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A neural commit that links code changes to their context.
///
/// This is the core data structure in AGIT's neural graph.
/// Each neural commit corresponds to a Git commit and captures
/// the "why" and "how" alongside the "what" (code changes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeuralCommit {
    /// The Git commit hash this neural commit is linked to.
    pub git_hash: String,

    /// The parent neural commit hash (for walking the neural graph).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_hash: Option<String>,

    /// Author of the commit.
    pub author: String,

    /// Hash of the roadmap blob.
    pub roadmap_hash: String,

    /// Hash of the trace blob (full index content).
    pub trace_hash: String,

    /// Synthesized summary of the change.
    pub summary: String,

    /// When this neural commit was created.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
}

impl NeuralCommit {
    /// Create a new neural commit.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        git_hash: impl Into<String>,
        parent_hash: Option<String>,
        author: impl Into<String>,
        roadmap_hash: impl Into<String>,
        trace_hash: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            git_hash: git_hash.into(),
            parent_hash,
            author: author.into(),
            roadmap_hash: roadmap_hash.into(),
            trace_hash: trace_hash.into(),
            summary: summary.into(),
            created_at: Utc::now(),
        }
    }

    /// Check if this is the first neural commit (no parent).
    pub fn is_root(&self) -> bool {
        self.parent_hash.is_none()
    }

    /// Get a short version of the git hash (first 7 characters).
    pub fn short_hash(&self) -> &str {
        if self.git_hash.len() >= 7 {
            &self.git_hash[..7]
        } else {
            &self.git_hash
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neural_commit_creation() {
        let commit = NeuralCommit::new(
            "abc123def456",
            None,
            "test@example.com",
            "roadmap_hash",
            "trace_hash",
            "Intent: Fix bug. Plan: Add validation.",
        );

        assert_eq!(commit.git_hash, "abc123def456");
        assert!(commit.is_root());
        assert_eq!(commit.short_hash(), "abc123d");
    }

    #[test]
    fn test_neural_commit_with_parent() {
        let commit = NeuralCommit::new(
            "abc123",
            Some("parent123".to_string()),
            "test@example.com",
            "roadmap",
            "trace",
            "Summary",
        );

        assert!(!commit.is_root());
        assert_eq!(commit.parent_hash, Some("parent123".to_string()));
    }

    #[test]
    fn test_neural_commit_serialization() {
        let commit = NeuralCommit::new(
            "abc123",
            None,
            "test@example.com",
            "roadmap",
            "trace",
            "Summary",
        );

        let json = serde_json::to_string(&commit).unwrap();
        assert!(json.contains("\"git_hash\":\"abc123\""));
        assert!(!json.contains("parent_hash")); // Should be skipped when None
    }
}
