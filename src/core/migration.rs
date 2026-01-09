//! Storage migration between V1 (file-based) and V2 (Git-native).
//!
//! V1: Objects in `.agit/objects/`, refs in `.agit/refs/heads/`
//! V2: Objects in Git ODB, refs in `refs/agit/heads/`

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use git2::{Oid, Repository};

use crate::error::Result;
use crate::storage::{FileObjectStore, FileRefStore, RefStore};

/// Storage version for migration tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageVersion {
    /// V1: File-based storage in `.agit/objects/` and `.agit/refs/heads/`
    V1FileSystem,
    /// V2: Git ODB for objects, `refs/agit/heads/` for refs
    V2GitNative,
    /// Mixed: Has both V1 and V2 data
    Mixed,
}

/// Result of a migration operation.
#[derive(Debug)]
pub struct MigrationResult {
    /// Number of objects migrated.
    pub objects_migrated: usize,
    /// Number of refs migrated.
    pub refs_migrated: usize,
    /// The resulting storage version.
    pub version: StorageVersion,
}

/// Detect the current storage version.
///
/// # Arguments
///
/// * `agit_dir` - Path to the .agit directory
/// * `repo` - Git repository reference
pub fn detect_version(agit_dir: &Path, repo: &Repository) -> StorageVersion {
    // Check for V2 refs (refs/agit/*)
    let has_git_refs = repo
        .references_glob("refs/agit/*")
        .map(|mut r| r.next().is_some())
        .unwrap_or(false);

    // Check for V1 refs (.agit/refs/heads/*)
    let refs_dir = agit_dir.join("refs").join("heads");
    let has_file_refs = refs_dir.exists()
        && refs_dir
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);

    // Check for V1 objects (.agit/objects/*)
    let objects_dir = agit_dir.join("objects");
    let has_file_objects = objects_dir.exists()
        && objects_dir
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);

    match (has_git_refs, has_file_refs || has_file_objects) {
        (true, false) => StorageVersion::V2GitNative,
        (false, true) => StorageVersion::V1FileSystem,
        (true, true) => StorageVersion::Mixed,
        (false, false) => StorageVersion::V2GitNative, // Empty repo defaults to V2
    }
}

/// Migrate from V1 (file-based) to V2 (Git-native) storage.
///
/// This operation:
/// 1. Copies all objects from `.agit/objects/` to Git ODB
/// 2. Creates `refs/agit/heads/*` refs from `.agit/refs/heads/*`
/// 3. Optionally cleans up the old storage (with `cleanup` flag)
///
/// # Arguments
///
/// * `agit_dir` - Path to the .agit directory
/// * `repo` - Git repository reference
pub fn migrate_v1_to_v2(agit_dir: &Path, repo: &Repository) -> Result<MigrationResult> {
    let file_refs = FileRefStore::new(agit_dir);

    // Map old hashes (SHA-256) to new hashes (Git SHA-1 OID)
    let mut hash_map: HashMap<String, String> = HashMap::new();
    let mut objects_migrated = 0;

    // Step 1: Migrate all objects from .agit/objects/ to Git ODB
    let objects_dir = agit_dir.join("objects");
    if objects_dir.exists() {
        for prefix_entry in fs::read_dir(&objects_dir)?.flatten() {
            if !prefix_entry.file_type()?.is_dir() {
                continue;
            }

            let prefix = prefix_entry.file_name();
            for obj_entry in fs::read_dir(prefix_entry.path())?.flatten() {
                let content = fs::read(obj_entry.path())?;

                // Store in Git ODB as a blob
                let oid = repo.blob(&content)?;

                // Build old hash from path components
                let suffix = obj_entry.file_name();
                let old_hash = format!(
                    "{}{}",
                    prefix.to_string_lossy(),
                    suffix.to_string_lossy()
                );

                hash_map.insert(old_hash, oid.to_string());
                objects_migrated += 1;
            }
        }
    }

    // Step 2: Migrate refs from .agit/refs/heads/ to refs/agit/heads/
    let mut refs_migrated = 0;
    for branch in file_refs.list()? {
        if let Some(old_hash) = file_refs.get(&branch)? {
            // Map old SHA-256 hash to new Git OID
            if let Some(new_oid_str) = hash_map.get(&old_hash) {
                let ref_name = format!("refs/agit/heads/{}", branch);
                let oid = Oid::from_str(new_oid_str)?;
                repo.reference(
                    &ref_name,
                    oid,
                    true,
                    &format!("agit migration: {}", branch),
                )?;
                refs_migrated += 1;
            } else {
                // Old hash not found in objects - might be a direct OID
                // Try to parse as OID directly
                if let Ok(oid) = Oid::from_str(&old_hash) {
                    let ref_name = format!("refs/agit/heads/{}", branch);
                    repo.reference(
                        &ref_name,
                        oid,
                        true,
                        &format!("agit migration: {}", branch),
                    )?;
                    refs_migrated += 1;
                }
            }
        }
    }

    Ok(MigrationResult {
        objects_migrated,
        refs_migrated,
        version: StorageVersion::V2GitNative,
    })
}

/// Clean up V1 storage after successful migration.
///
/// Removes `.agit/objects/` and `.agit/refs/` directories.
pub fn cleanup_v1_storage(agit_dir: &Path) -> Result<()> {
    let objects_dir = agit_dir.join("objects");
    if objects_dir.exists() {
        fs::remove_dir_all(&objects_dir)?;
    }

    let refs_dir = agit_dir.join("refs");
    if refs_dir.exists() {
        fs::remove_dir_all(&refs_dir)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Repository) {
        let temp = TempDir::new().unwrap();
        let repo = Repository::init(temp.path()).unwrap();
        (temp, repo)
    }

    #[test]
    fn test_detect_version_empty() {
        let (temp, repo) = setup();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();

        // Empty .agit/ defaults to V2
        let version = detect_version(&agit_dir, &repo);
        assert_eq!(version, StorageVersion::V2GitNative);
    }

    #[test]
    fn test_detect_version_v1() {
        let (temp, repo) = setup();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(agit_dir.join("objects").join("ab")).unwrap();
        fs::write(
            agit_dir.join("objects").join("ab").join("cdef"),
            "content",
        )
        .unwrap();

        let version = detect_version(&agit_dir, &repo);
        assert_eq!(version, StorageVersion::V1FileSystem);
    }

    #[test]
    fn test_detect_version_v2() {
        let (temp, repo) = setup();
        let agit_dir = temp.path().join(".agit");
        fs::create_dir_all(&agit_dir).unwrap();

        // Create a blob and a ref in V2 format
        let oid = repo.blob(b"test content").unwrap();
        repo.reference("refs/agit/heads/main", oid, true, "test")
            .unwrap();

        let version = detect_version(&agit_dir, &repo);
        assert_eq!(version, StorageVersion::V2GitNative);
    }

    #[test]
    fn test_migrate_v1_to_v2() {
        let (temp, repo) = setup();
        let agit_dir = temp.path().join(".agit");

        // Set up V1 storage
        let objects_dir = agit_dir.join("objects").join("ab");
        fs::create_dir_all(&objects_dir).unwrap();

        // Create a test object with known hash prefix
        let test_content = b"{\"test\": \"content\"}";
        let hash = FileObjectStore::hash_content(test_content);
        let (prefix, rest) = hash.split_at(2);
        fs::create_dir_all(agit_dir.join("objects").join(prefix)).unwrap();
        fs::write(agit_dir.join("objects").join(prefix).join(rest), test_content).unwrap();

        // Create V1 ref pointing to the object
        let refs_dir = agit_dir.join("refs").join("heads");
        fs::create_dir_all(&refs_dir).unwrap();
        fs::write(refs_dir.join("main"), &hash).unwrap();

        // Migrate
        let result = migrate_v1_to_v2(&agit_dir, &repo).unwrap();
        assert_eq!(result.objects_migrated, 1);
        assert_eq!(result.refs_migrated, 1);

        // Verify V2 ref exists
        let v2_ref = repo.find_reference("refs/agit/heads/main").unwrap();
        assert!(v2_ref.target().is_some());
    }

    #[test]
    fn test_cleanup_v1_storage() {
        let (temp, _repo) = setup();
        let agit_dir = temp.path().join(".agit");

        // Create V1 directories
        fs::create_dir_all(agit_dir.join("objects").join("ab")).unwrap();
        fs::create_dir_all(agit_dir.join("refs").join("heads")).unwrap();

        assert!(agit_dir.join("objects").exists());
        assert!(agit_dir.join("refs").exists());

        // Cleanup
        cleanup_v1_storage(&agit_dir).unwrap();

        assert!(!agit_dir.join("objects").exists());
        assert!(!agit_dir.join("refs").exists());
    }
}
