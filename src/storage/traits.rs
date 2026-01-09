//! Storage trait definitions.
//!
//! These traits define the interface for storage operations,
//! enabling dependency injection and easier testing.

use crate::domain::IndexEntry;
use crate::error::Result;

/// Content-addressable object storage trait.
///
/// Objects are stored by their content hash (SHA-256).
/// This is similar to how Git stores objects.
pub trait ObjectStore: Send + Sync {
    /// Save content and return its hash.
    ///
    /// The content is stored in `.agit/objects/{hash[0..2]}/{hash[2..]}`.
    fn save(&self, content: &[u8]) -> Result<String>;

    /// Load content by its hash.
    fn load(&self, hash: &str) -> Result<Vec<u8>>;

    /// Check if an object with the given hash exists.
    fn exists(&self, hash: &str) -> Result<bool>;

    /// Delete an object by its hash.
    fn delete(&self, hash: &str) -> Result<()>;
}

/// Index (staging area) storage trait.
///
/// The index stores entries as JSONL (JSON Lines) in `.agit/index`.
pub trait IndexStore: Send + Sync {
    /// Append an entry to the index.
    fn append(&self, entry: &IndexEntry) -> Result<()>;

    /// Read all entries from the index.
    fn read_all(&self) -> Result<Vec<IndexEntry>>;

    /// Clear all entries from the index.
    fn clear(&self) -> Result<()>;

    /// Count the number of entries in the index.
    fn count(&self) -> Result<usize>;

    /// Check if the index is empty.
    fn is_empty(&self) -> Result<bool> {
        Ok(self.count()? == 0)
    }
}

/// Branch reference storage trait.
///
/// Refs are stored in `.agit/refs/heads/{branch_name}` and point
/// to the hash of the latest neural commit on that branch.
pub trait RefStore: Send + Sync {
    /// Get the hash that a ref points to.
    fn get(&self, ref_name: &str) -> Result<Option<String>>;

    /// Update a ref to point to a new hash.
    fn update(&self, ref_name: &str, hash: &str) -> Result<()>;

    /// Delete a ref.
    fn delete(&self, ref_name: &str) -> Result<()>;

    /// List all refs.
    fn list(&self) -> Result<Vec<String>>;
}

/// HEAD pointer storage trait.
///
/// HEAD stores the name of the current branch in `.agit/HEAD`.
pub trait HeadStore: Send + Sync {
    /// Get the current branch name.
    fn get(&self) -> Result<Option<String>>;

    /// Set the current branch name.
    fn set(&self, branch: &str) -> Result<()>;
}
