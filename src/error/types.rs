//! Error type definitions for AGIT.
//!
//! Provides structured error types with descriptive messages for all
//! failure modes in the application.

use std::path::PathBuf;
use thiserror::Error;

/// Top-level error type for AGIT operations.
///
/// This enum covers all possible error conditions that can occur
/// during AGIT operations, from initialization to commit workflows.
#[derive(Error, Debug)]
pub enum AgitError {
    /// The current directory is not an AGIT repository.
    #[error("Repository not initialized. Run 'agit init' first.")]
    NotInitialized,

    /// Attempted to initialize in an already initialized repository.
    #[error("Repository already initialized at {}", path.display())]
    AlreadyInitialized {
        /// Path to the existing .agit directory.
        path: PathBuf,
    },

    /// The current directory is not a Git repository.
    #[error("Not a git repository. Run 'git init' first.")]
    NotGitRepository,

    /// Failed to acquire the lock file.
    #[error("Lock acquisition failed: {reason}")]
    LockFailed {
        /// Reason for the lock failure.
        reason: String,
    },

    /// A Git operation failed.
    #[error("Git operation failed: {0}")]
    Git(#[from] git2::Error),

    /// A storage operation failed.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// An index operation failed.
    #[error("Index error: {0}")]
    Index(#[from] IndexError),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// An I/O operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Tantivy search error.
    #[error("Search error: {0}")]
    Search(#[from] tantivy::TantivyError),

    /// Tantivy query parser error.
    #[error("Query parse error: {0}")]
    QueryParse(#[from] tantivy::query::QueryParserError),

    /// Invalid command arguments.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Operation was cancelled by the user.
    #[error("Operation cancelled")]
    Cancelled,

    /// Nothing to commit (no code or memory changes).
    #[error(
        "Nothing to commit. Record thoughts with 'agit record' or stage code with 'agit add'."
    )]
    NothingToCommit,

    /// Semantic conflict detected between external commits and pending thoughts.
    ///
    /// This occurs when external git commits (made outside of Agit) modified files
    /// that are mentioned in pending thoughts. Committing would create "hallucinated"
    /// context that contradicts the actual codebase state.
    #[error("Context conflict: external changes touched files in your pending thoughts.\nConflicting files: {}\nUse --force to commit anyway, or clear pending thoughts with 'agit reset'.", files.join(", "))]
    SemanticConflict {
        /// Files that were both modified externally and mentioned in pending thoughts.
        files: Vec<String>,
    },

    /// A Git merge or rebase is in progress.
    ///
    /// Agit cannot safely modify the neural graph while Git is in a conflicted
    /// state. The user must resolve the conflicts and complete the operation first.
    #[error("⚠️  {operation} in progress. Please resolve conflicts and finish the {operation} before adding neural memories.")]
    ConflictedState {
        /// The type of Git operation in progress (e.g., "Merge", "Rebase").
        operation: String,
    },

    /// Path is outside the repository boundary.
    ///
    /// This occurs when attempting to log context for a file that is not
    /// within the current repository root. Agit is a single-repo tool and
    /// requires switching directories to log context for other repositories.
    #[error("⛔ SECURITY VIOLATION: Path '{path}' is outside repository scope ({repo_root}). Agit is a single-repo tool. Switch to the correct directory to log context for external files.")]
    PathOutsideRepository {
        /// The path that was outside the repository.
        path: String,
        /// The repository root path.
        repo_root: String,
    },

    /// File does not exist in the repository.
    ///
    /// This occurs when attempting to log context for a file path that
    /// does not exist. Log reasoning after creating files, not before.
    #[error("File '{path}' does not exist in repository ({repo_root}). Only existing files can be referenced in location metadata.")]
    FileNotFound {
        /// The path that was not found.
        path: String,
        /// The repository root path.
        repo_root: String,
    },
}

/// Errors specific to the content-addressable storage layer.
#[derive(Error, Debug)]
pub enum StorageError {
    /// The requested object was not found in storage.
    #[error("Object not found: {hash}")]
    NotFound {
        /// The hash of the missing object.
        hash: String,
    },

    /// The object data is corrupted or invalid.
    #[error("Corrupt object {hash}: {reason}")]
    Corrupt {
        /// The hash of the corrupt object.
        hash: String,
        /// Description of the corruption.
        reason: String,
    },

    /// Failed to write an object to storage.
    #[error("Write failed: {0}")]
    WriteFailed(String),

    /// Failed to read an object from storage.
    #[error("Read failed: {0}")]
    ReadFailed(String),

    /// Invalid hash format.
    #[error("Invalid hash format: {0}")]
    InvalidHash(String),
}

/// Errors specific to the index (staging area) operations.
#[derive(Error, Debug)]
pub enum IndexError {
    /// The index is empty when entries were expected.
    #[error("Index is empty")]
    Empty,

    /// An entry in the index is malformed.
    #[error("Malformed entry at line {line}: {reason}")]
    MalformedEntry {
        /// Line number (1-indexed) of the malformed entry.
        line: usize,
        /// Description of the parsing error.
        reason: String,
    },

    /// Failed to append to the index.
    #[error("Failed to append entry: {0}")]
    AppendFailed(String),

    /// Failed to clear the index.
    #[error("Failed to clear index: {0}")]
    ClearFailed(String),
}

/// Result type alias using [`AgitError`].
pub type Result<T> = std::result::Result<T, AgitError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AgitError::NotInitialized;
        assert_eq!(
            err.to_string(),
            "Repository not initialized. Run 'agit init' first."
        );

        let err = AgitError::AlreadyInitialized {
            path: PathBuf::from("/test/.agit"),
        };
        assert!(err.to_string().contains("/test/.agit"));
    }

    #[test]
    fn test_storage_error_display() {
        let err = StorageError::NotFound {
            hash: "abc123".to_string(),
        };
        assert_eq!(err.to_string(), "Object not found: abc123");
    }

    #[test]
    fn test_index_error_display() {
        let err = IndexError::MalformedEntry {
            line: 42,
            reason: "invalid JSON".to_string(),
        };
        assert_eq!(err.to_string(), "Malformed entry at line 42: invalid JSON");
    }
}
