//! Merge-friendly immutable memory storage.
//!
//! This module provides a content-addressable storage system for memory nodes,
//! designed to be merge-friendly by storing each memory as a separate immutable
//! JSON file. This eliminates merge conflicts when multiple users generate
//! different memories on different branches.
//!
//! ## Storage Model
//!
//! Similar to Git's internal blob storage:
//! - Each memory is stored as a separate `.json` file
//! - Files are organized in sharded directories: `.agit/objects/memories/{hash[0..2]}/{hash[2..]}.json`
//! - Content is hashed with SHA-256 for deduplication and integrity
//! - Files are immutable once written (idempotent saves)
//!
//! ## Merge Behavior
//!
//! When two branches have different memories:
//! - Git merge keeps both files (union merge)
//! - No conflicts occur since files don't overlap
//! - `get_all_memories()` returns the union of all memories

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::{Category, IndexEntry, Location, Role};
use crate::error::{AgitError, Result, StorageError};
use crate::safety::atomic_write;

// ============================================================================
// MemoryNode - The Core Data Structure
// ============================================================================

/// A memory node representing a single thought, intent, or reasoning entry.
///
/// Unlike `IndexEntry` which is transient (stored in staging area), `MemoryNode`
/// is the permanent, immutable representation of a memory linked to a commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryNode {
    /// The git commit hash this memory is associated with.
    /// Empty string for uncommitted memories.
    #[serde(default)]
    pub commit_hash: String,

    /// Who created this entry (user or AI).
    pub role: Role,

    /// Category of the entry (intent, reasoning, error, note).
    pub category: Category,

    /// The actual content/message.
    pub content: String,

    /// When this memory was created.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,

    /// Code locations this memory relates to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<Location>>,

    /// Schema version for future migrations.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

fn default_schema_version() -> u32 {
    1
}

impl MemoryNode {
    /// Create a new memory node with the current timestamp.
    pub fn new(
        commit_hash: impl Into<String>,
        role: Role,
        category: Category,
        content: impl Into<String>,
    ) -> Self {
        Self {
            commit_hash: commit_hash.into(),
            role,
            category,
            content: content.into(),
            timestamp: Utc::now(),
            locations: None,
            schema_version: 1,
        }
    }

    /// Create a memory node with locations.
    pub fn with_locations(
        commit_hash: impl Into<String>,
        role: Role,
        category: Category,
        content: impl Into<String>,
        locations: Vec<Location>,
    ) -> Self {
        Self {
            commit_hash: commit_hash.into(),
            role,
            category,
            content: content.into(),
            timestamp: Utc::now(),
            locations: if locations.is_empty() {
                None
            } else {
                Some(locations)
            },
            schema_version: 1,
        }
    }

    /// Convert from an IndexEntry, associating with a commit.
    pub fn from_index_entry(entry: &IndexEntry, commit_hash: impl Into<String>) -> Self {
        Self {
            commit_hash: commit_hash.into(),
            role: entry.role,
            category: entry.category,
            content: entry.content.clone(),
            timestamp: entry.timestamp,
            locations: entry.locations.clone(),
            schema_version: 1,
        }
    }

    /// Get all locations, returning empty vec if none.
    pub fn get_locations(&self) -> Vec<Location> {
        self.locations.clone().unwrap_or_default()
    }

    /// Compute the SHA-256 hash of this memory's JSON representation.
    pub fn compute_hash(&self) -> Result<String> {
        let json = serde_json::to_vec(self)?;
        let mut hasher = Sha256::new();
        hasher.update(&json);
        Ok(hex::encode(hasher.finalize()))
    }
}

// ============================================================================
// MemoryStore Trait
// ============================================================================

/// Trait for memory storage operations.
///
/// Provides a merge-friendly storage interface where each memory is stored
/// as a separate immutable file.
pub trait MemoryStore: Send + Sync {
    /// Save a memory node to storage.
    ///
    /// Returns the content hash (SHA-256) of the stored memory.
    /// If the memory already exists (same hash), this is a no-op.
    fn save_memory(&self, memory: &MemoryNode) -> Result<String>;

    /// Load a specific memory by its hash.
    fn load_memory(&self, hash: &str) -> Result<MemoryNode>;

    /// Check if a memory exists by its hash.
    fn exists(&self, hash: &str) -> Result<bool>;

    /// Get all memories in storage.
    ///
    /// Traverses the entire storage directory and deserializes all memory files.
    fn get_all_memories(&self) -> Result<Vec<MemoryNode>>;

    /// Get all memories associated with a specific commit.
    ///
    /// More efficient than loading all memories when you only need
    /// memories for a specific commit.
    fn get_memories_by_commit(&self, commit_hash: &str) -> Result<Vec<MemoryNode>>;

    /// Get all memory hashes in storage.
    ///
    /// Useful for listing without loading full content.
    fn list_hashes(&self) -> Result<Vec<String>>;

    /// Delete a memory by its hash.
    fn delete(&self, hash: &str) -> Result<()>;

    /// Get the count of memories in storage.
    fn count(&self) -> Result<usize>;
}

// ============================================================================
// FileMemoryStore Implementation
// ============================================================================

/// File-system based memory store with SHA-256 sharded directories.
///
/// Storage layout:
/// ```text
/// .agit/
/// └── objects/
///     └── memories/
///         ├── a1/
///         │   ├── b2c3d4e5...json
///         │   └── f6g7h8i9...json
///         ├── b2/
///         │   └── ...
///         └── ...
/// ```
pub struct FileMemoryStore {
    /// Path to the memories directory (`.agit/objects/memories`).
    memories_dir: PathBuf,
}

impl FileMemoryStore {
    /// Create a new file memory store.
    ///
    /// # Arguments
    /// * `agit_dir` - Path to the `.agit` directory
    pub fn new(agit_dir: &Path) -> Self {
        Self {
            memories_dir: agit_dir.join("objects").join("memories"),
        }
    }

    /// Compute the SHA-256 hash of content bytes.
    pub fn hash_content(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    /// Get the path where a memory would be stored based on its hash.
    ///
    /// Uses 2-character prefix sharding like Git:
    /// - Hash: `a1b2c3d4e5...`
    /// - Path: `.agit/objects/memories/a1/b2c3d4e5...json`
    fn memory_path(&self, hash: &str) -> Result<PathBuf> {
        if hash.len() < 4 {
            return Err(AgitError::Storage(StorageError::InvalidHash(
                hash.to_string(),
            )));
        }
        let (prefix, rest) = hash.split_at(2);
        Ok(self.memories_dir.join(prefix).join(format!("{}.json", rest)))
    }

    /// Ensure the parent directory exists for a given path.
    fn ensure_parent_dir(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    /// Iterate over all memory files in storage.
    ///
    /// Yields tuples of (hash, file_path) for each memory file found.
    fn iter_memory_files(&self) -> Result<Vec<(String, PathBuf)>> {
        let mut results = Vec::new();

        // Check if memories directory exists
        if !self.memories_dir.exists() {
            return Ok(results);
        }

        // Iterate over shard directories (00-ff)
        for shard_entry in fs::read_dir(&self.memories_dir)? {
            let shard_entry = shard_entry?;
            let shard_path = shard_entry.path();

            if !shard_path.is_dir() {
                continue;
            }

            let shard_name = shard_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Must be a 2-character hex prefix
            if shard_name.len() != 2 {
                continue;
            }

            // Iterate over memory files in this shard
            for file_entry in fs::read_dir(&shard_path)? {
                let file_entry = file_entry?;
                let file_path = file_entry.path();

                if !file_path.is_file() {
                    continue;
                }

                // Must be a .json file
                let file_name = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                if !file_name.ends_with(".json") {
                    continue;
                }

                // Reconstruct the full hash from shard + filename
                let rest = file_name.trim_end_matches(".json");
                let full_hash = format!("{}{}", shard_name, rest);

                results.push((full_hash, file_path));
            }
        }

        Ok(results)
    }
}

impl MemoryStore for FileMemoryStore {
    fn save_memory(&self, memory: &MemoryNode) -> Result<String> {
        // Serialize to JSON
        let json = serde_json::to_vec_pretty(memory)?;

        // Compute hash
        let hash = Self::hash_content(&json);

        // Get storage path
        let path = self.memory_path(&hash)?;

        // Check if already exists (idempotent)
        if path.exists() {
            return Ok(hash);
        }

        // Ensure parent directory exists
        self.ensure_parent_dir(&path)?;

        // Write atomically
        atomic_write(&path, &json)?;

        Ok(hash)
    }

    fn load_memory(&self, hash: &str) -> Result<MemoryNode> {
        let path = self.memory_path(hash)?;

        if !path.exists() {
            return Err(AgitError::Storage(StorageError::NotFound {
                hash: hash.to_string(),
            }));
        }

        let content = fs::read(&path).map_err(|e| {
            AgitError::Storage(StorageError::ReadFailed(format!(
                "Failed to read memory {}: {}",
                hash, e
            )))
        })?;

        let memory: MemoryNode = serde_json::from_slice(&content).map_err(|e| {
            AgitError::Storage(StorageError::Corrupt {
                hash: hash.to_string(),
                reason: e.to_string(),
            })
        })?;

        Ok(memory)
    }

    fn exists(&self, hash: &str) -> Result<bool> {
        let path = self.memory_path(hash)?;
        Ok(path.exists())
    }

    fn get_all_memories(&self) -> Result<Vec<MemoryNode>> {
        let files = self.iter_memory_files()?;
        let mut memories = Vec::with_capacity(files.len());

        for (hash, path) in files {
            let content = fs::read(&path).map_err(|e| {
                AgitError::Storage(StorageError::ReadFailed(format!(
                    "Failed to read memory {}: {}",
                    hash, e
                )))
            })?;

            match serde_json::from_slice::<MemoryNode>(&content) {
                Ok(memory) => memories.push(memory),
                Err(e) => {
                    // Log warning but continue - don't fail on corrupt files
                    tracing::warn!("Skipping corrupt memory file {}: {}", hash, e);
                }
            }
        }

        // Sort by timestamp for consistent ordering
        memories.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(memories)
    }

    fn get_memories_by_commit(&self, commit_hash: &str) -> Result<Vec<MemoryNode>> {
        // For now, we load all memories and filter
        // Future optimization: maintain an index file mapping commit_hash -> memory_hashes
        let all_memories = self.get_all_memories()?;

        Ok(all_memories
            .into_iter()
            .filter(|m| m.commit_hash == commit_hash)
            .collect())
    }

    fn list_hashes(&self) -> Result<Vec<String>> {
        let files = self.iter_memory_files()?;
        Ok(files.into_iter().map(|(hash, _)| hash).collect())
    }

    fn delete(&self, hash: &str) -> Result<()> {
        let path = self.memory_path(hash)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn count(&self) -> Result<usize> {
        let files = self.iter_memory_files()?;
        Ok(files.len())
    }
}

// ============================================================================
// Migration from Legacy Storage
// ============================================================================

/// Migrates memories from legacy JSONL index to immutable object storage.
///
/// This function handles the transition from the old single-file format
/// (`.agit/log.json` or `.agit/index`) to the new sharded object storage.
pub struct MemoryMigration {
    memory_store: FileMemoryStore,
    agit_dir: PathBuf,
}

impl MemoryMigration {
    /// Create a new migration handler.
    pub fn new(agit_dir: &Path) -> Self {
        Self {
            memory_store: FileMemoryStore::new(agit_dir),
            agit_dir: agit_dir.to_path_buf(),
        }
    }

    /// Check if legacy storage exists and needs migration.
    pub fn needs_migration(&self) -> bool {
        self.legacy_log_path().exists()
    }

    /// Path to legacy log file.
    fn legacy_log_path(&self) -> PathBuf {
        self.agit_dir.join("log.json")
    }

    /// Migrate from legacy `log.json` to new object storage.
    ///
    /// Returns the number of memories migrated.
    pub fn migrate(&self) -> Result<usize> {
        let legacy_path = self.legacy_log_path();

        if !legacy_path.exists() {
            return Ok(0);
        }

        // Read legacy file
        let content = fs::read_to_string(&legacy_path)?;

        // Try parsing as array of entries first
        let entries: Vec<LegacyLogEntry> = if content.trim().starts_with('[') {
            // JSON array format
            serde_json::from_str(&content)?
        } else {
            // JSONL format (one JSON object per line)
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| serde_json::from_str(line))
                .collect::<std::result::Result<Vec<_>, _>>()?
        };

        let count = entries.len();

        // Convert and save each entry
        for entry in entries {
            let memory = entry.to_memory_node();
            self.memory_store.save_memory(&memory)?;
        }

        // Remove legacy file after successful migration
        fs::remove_file(&legacy_path)?;

        tracing::info!("Migrated {} memories from legacy storage", count);

        Ok(count)
    }

    /// Migrate staged index entries to memory storage.
    ///
    /// This is called during commit to convert staging entries to permanent memories.
    pub fn migrate_index_entries(
        &self,
        entries: &[IndexEntry],
        commit_hash: &str,
    ) -> Result<Vec<String>> {
        let mut hashes = Vec::with_capacity(entries.len());

        for entry in entries {
            let memory = MemoryNode::from_index_entry(entry, commit_hash);
            let hash = self.memory_store.save_memory(&memory)?;
            hashes.push(hash);
        }

        Ok(hashes)
    }
}

/// Legacy log entry format for migration.
#[derive(Debug, Deserialize)]
struct LegacyLogEntry {
    #[serde(default)]
    commit_hash: Option<String>,
    role: Role,
    category: Category,
    content: String,
    #[serde(default = "Utc::now", with = "chrono::serde::ts_seconds")]
    timestamp: DateTime<Utc>,
    #[serde(default)]
    locations: Option<Vec<Location>>,
    #[serde(default)]
    file_path: Option<String>,
    #[serde(default)]
    line_number: Option<u32>,
}

impl LegacyLogEntry {
    fn to_memory_node(&self) -> MemoryNode {
        // Normalize legacy file_path/line_number to locations
        let locations = if self.locations.is_some() {
            self.locations.clone()
        } else if let Some(ref path) = self.file_path {
            Some(vec![Location {
                file: path.clone(),
                start_line: self.line_number,
                end_line: None,
            }])
        } else {
            None
        };

        MemoryNode {
            commit_hash: self.commit_hash.clone().unwrap_or_default(),
            role: self.role,
            category: self.category,
            content: self.content.clone(),
            timestamp: self.timestamp,
            locations,
            schema_version: 1,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FileMemoryStore) {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();
        let store = FileMemoryStore::new(&agit_dir);
        (temp, store)
    }

    #[test]
    fn test_memory_node_creation() {
        let memory = MemoryNode::new(
            "abc123",
            Role::User,
            Category::Intent,
            "Fix the auth bug",
        );

        assert_eq!(memory.commit_hash, "abc123");
        assert_eq!(memory.role, Role::User);
        assert_eq!(memory.category, Category::Intent);
        assert_eq!(memory.content, "Fix the auth bug");
        assert_eq!(memory.schema_version, 1);
    }

    #[test]
    fn test_memory_node_with_locations() {
        let locations = vec![
            Location::file("src/auth.rs"),
            Location::range("src/main.rs", 10, 20),
        ];

        let memory = MemoryNode::with_locations(
            "abc123",
            Role::Ai,
            Category::Reasoning,
            "Added error handling",
            locations,
        );

        assert_eq!(memory.get_locations().len(), 2);
    }

    #[test]
    fn test_memory_node_serialization() {
        let memory = MemoryNode::new(
            "abc123",
            Role::User,
            Category::Intent,
            "Test content",
        );

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"commit_hash\":\"abc123\""));
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"category\":\"intent\""));
    }

    #[test]
    fn test_memory_node_hash() {
        let memory = MemoryNode::new(
            "abc123",
            Role::User,
            Category::Intent,
            "Test content",
        );

        let hash = memory.compute_hash().unwrap();
        assert_eq!(hash.len(), 64); // SHA-256 = 64 hex chars
    }

    #[test]
    fn test_save_and_load_memory() {
        let (_temp, store) = setup();

        let memory = MemoryNode::new(
            "commit123",
            Role::User,
            Category::Intent,
            "Fix the bug",
        );

        let hash = store.save_memory(&memory).unwrap();
        assert!(store.exists(&hash).unwrap());

        let loaded = store.load_memory(&hash).unwrap();
        assert_eq!(loaded.content, memory.content);
        assert_eq!(loaded.commit_hash, memory.commit_hash);
    }

    #[test]
    fn test_save_is_idempotent() {
        let (_temp, store) = setup();

        let memory = MemoryNode::new(
            "commit123",
            Role::User,
            Category::Intent,
            "Same content",
        );

        let hash1 = store.save_memory(&memory).unwrap();
        let hash2 = store.save_memory(&memory).unwrap();

        assert_eq!(hash1, hash2);
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn test_get_all_memories() {
        let (_temp, store) = setup();

        // Save multiple memories
        for i in 0..5 {
            let memory = MemoryNode::new(
                format!("commit{}", i),
                Role::User,
                Category::Intent,
                format!("Content {}", i),
            );
            store.save_memory(&memory).unwrap();
        }

        let all = store.get_all_memories().unwrap();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_get_memories_by_commit() {
        let (_temp, store) = setup();

        // Save memories for different commits
        let memory1 = MemoryNode::new("commit_a", Role::User, Category::Intent, "Intent A");
        let memory2 = MemoryNode::new("commit_a", Role::Ai, Category::Reasoning, "Reasoning A");
        let memory3 = MemoryNode::new("commit_b", Role::User, Category::Intent, "Intent B");

        store.save_memory(&memory1).unwrap();
        store.save_memory(&memory2).unwrap();
        store.save_memory(&memory3).unwrap();

        let commit_a_memories = store.get_memories_by_commit("commit_a").unwrap();
        assert_eq!(commit_a_memories.len(), 2);

        let commit_b_memories = store.get_memories_by_commit("commit_b").unwrap();
        assert_eq!(commit_b_memories.len(), 1);
    }

    #[test]
    fn test_list_hashes() {
        let (_temp, store) = setup();

        let memory = MemoryNode::new("commit1", Role::User, Category::Intent, "Test");
        let hash = store.save_memory(&memory).unwrap();

        let hashes = store.list_hashes().unwrap();
        assert_eq!(hashes.len(), 1);
        assert!(hashes.contains(&hash));
    }

    #[test]
    fn test_delete_memory() {
        let (_temp, store) = setup();

        let memory = MemoryNode::new("commit1", Role::User, Category::Intent, "To delete");
        let hash = store.save_memory(&memory).unwrap();

        assert!(store.exists(&hash).unwrap());
        store.delete(&hash).unwrap();
        assert!(!store.exists(&hash).unwrap());
    }

    #[test]
    fn test_from_index_entry() {
        let entry = IndexEntry::user_intent("Fix the authentication");
        let memory = MemoryNode::from_index_entry(&entry, "abc123");

        assert_eq!(memory.commit_hash, "abc123");
        assert_eq!(memory.role, Role::User);
        assert_eq!(memory.category, Category::Intent);
        assert_eq!(memory.content, "Fix the authentication");
    }

    #[test]
    fn test_empty_storage() {
        let (_temp, store) = setup();

        assert_eq!(store.count().unwrap(), 0);
        assert!(store.get_all_memories().unwrap().is_empty());
        assert!(store.list_hashes().unwrap().is_empty());
    }

    #[test]
    fn test_memory_path_sharding() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        let store = FileMemoryStore::new(&agit_dir);

        // Hash with known prefix
        let hash = "a1b2c3d4e5f6789012345678901234567890123456789012345678901234";
        let path = store.memory_path(hash).unwrap();

        // Should be sharded: .agit/objects/memories/a1/b2c3d4...json
        assert!(path.to_string_lossy().contains("a1"));
        assert!(path.to_string_lossy().ends_with(".json"));
    }

    #[test]
    fn test_invalid_hash() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        let store = FileMemoryStore::new(&agit_dir);

        let result = store.memory_path("ab"); // Too short
        assert!(matches!(
            result,
            Err(AgitError::Storage(StorageError::InvalidHash(_)))
        ));
    }

    #[test]
    fn test_migration_needs_migration() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();

        let migration = MemoryMigration::new(&agit_dir);
        assert!(!migration.needs_migration());

        // Create legacy file
        fs::write(agit_dir.join("log.json"), "[]").unwrap();
        assert!(migration.needs_migration());
    }

    #[test]
    fn test_migration_jsonl_format() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();

        // Create legacy JSONL file
        let legacy_content = r#"{"role":"user","category":"intent","content":"First entry","timestamp":1704812345}
{"role":"ai","category":"reasoning","content":"Second entry","timestamp":1704812346}"#;

        fs::write(agit_dir.join("log.json"), legacy_content).unwrap();

        let migration = MemoryMigration::new(&agit_dir);
        let count = migration.migrate().unwrap();

        assert_eq!(count, 2);
        assert!(!migration.needs_migration()); // Legacy file should be deleted

        // Verify memories were saved
        assert_eq!(migration.memory_store.count().unwrap(), 2);
    }

    #[test]
    fn test_migration_json_array_format() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();

        // Create legacy JSON array file
        let legacy_content = r#"[
            {"role":"user","category":"intent","content":"First","timestamp":1704812345},
            {"role":"ai","category":"reasoning","content":"Second","timestamp":1704812346}
        ]"#;

        fs::write(agit_dir.join("log.json"), legacy_content).unwrap();

        let migration = MemoryMigration::new(&agit_dir);
        let count = migration.migrate().unwrap();

        assert_eq!(count, 2);
    }

    #[test]
    fn test_migrate_index_entries() {
        let temp = TempDir::new().unwrap();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();

        let migration = MemoryMigration::new(&agit_dir);

        let entries = vec![
            IndexEntry::user_intent("Fix bug"),
            IndexEntry::ai_reasoning("Added try/catch"),
        ];

        let hashes = migration.migrate_index_entries(&entries, "commit123").unwrap();

        assert_eq!(hashes.len(), 2);

        // Verify memories are stored with commit_hash
        let memories = migration.memory_store.get_memories_by_commit("commit123").unwrap();
        assert_eq!(memories.len(), 2);
    }
}
