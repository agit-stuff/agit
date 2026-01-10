//! Path validation utilities for repository boundary enforcement.
//!
//! This module provides functions to validate that file paths are within
//! the repository boundary, preventing cross-repository contamination.

use std::path::{Path, PathBuf};

use crate::error::{AgitError, Result};

/// Validates that a target path is within the repository root.
///
/// Returns the canonicalized path if valid, or an error if the path
/// escapes the repository boundary.
///
/// # Arguments
/// * `repo_root` - The repository root directory (where .git/.agit lives)
/// * `target_path` - The file path to validate (can be relative or absolute)
///
/// # Returns
/// * `Ok(PathBuf)` - The canonicalized, validated path
/// * `Err(AgitError::PathOutsideRepository)` - If path escapes repo boundary
///
/// # Examples
/// ```ignore
/// use std::path::Path;
/// use agit::safety::validate_path_is_internal;
///
/// let repo_root = Path::new("/home/user/project");
///
/// // Valid relative path
/// assert!(validate_path_is_internal(repo_root, "src/main.rs").is_ok());
///
/// // Path traversal attack - blocked
/// assert!(validate_path_is_internal(repo_root, "../other-repo/file.rs").is_err());
///
/// // Absolute path outside repo - blocked
/// assert!(validate_path_is_internal(repo_root, "/etc/passwd").is_err());
/// ```
pub fn validate_path_is_internal(repo_root: &Path, target_path: &str) -> Result<PathBuf> {
    // Canonicalize repo root
    let canonical_root = repo_root.canonicalize().map_err(|e| {
        AgitError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Cannot canonicalize repo root: {}", e),
        ))
    })?;

    // Resolve target path relative to repo root
    let resolved = if Path::new(target_path).is_absolute() {
        PathBuf::from(target_path)
    } else {
        repo_root.join(target_path)
    };

    // Try to canonicalize (file must exist or parent must exist)
    let canonical_target = if resolved.exists() {
        resolved.canonicalize()
    } else if let Some(parent) = resolved.parent() {
        if parent.exists() {
            // File doesn't exist yet, but parent does - canonicalize parent
            parent
                .canonicalize()
                .map(|p| p.join(resolved.file_name().unwrap_or_default()))
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Parent directory does not exist",
            ))
        }
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Cannot resolve path",
        ))
    }
    .map_err(|e| {
        AgitError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Cannot canonicalize target path '{}': {}", target_path, e),
        ))
    })?;

    // Check if target is within repo root
    if !canonical_target.starts_with(&canonical_root) {
        return Err(AgitError::PathOutsideRepository {
            path: target_path.to_string(),
            repo_root: canonical_root.display().to_string(),
        });
    }

    Ok(canonical_target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_valid_relative_path() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("file.rs"), "").unwrap();

        let result = validate_path_is_internal(temp.path(), "file.rs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_nested_path() {
        let temp = TempDir::new().unwrap();
        let src_dir = temp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("main.rs"), "").unwrap();

        let result = validate_path_is_internal(temp.path(), "src/main.rs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_path_traversal_blocked() {
        let temp = TempDir::new().unwrap();

        let result = validate_path_is_internal(temp.path(), "../outside.rs");
        assert!(result.is_err());

        if let Err(AgitError::PathOutsideRepository { path, .. }) = result {
            assert_eq!(path, "../outside.rs");
        } else {
            panic!("Expected PathOutsideRepository error");
        }
    }

    #[test]
    fn test_absolute_path_inside_allowed() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("file.rs"), "").unwrap();

        let abs_path = temp.path().join("file.rs");
        let result = validate_path_is_internal(temp.path(), abs_path.to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_absolute_path_outside_blocked() {
        let temp = TempDir::new().unwrap();

        // Use a path that definitely exists but is outside the temp dir
        #[cfg(unix)]
        let outside_path = "/tmp";
        #[cfg(windows)]
        let outside_path = "C:\\Windows";

        let result = validate_path_is_internal(temp.path(), outside_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_nonexistent_file_in_existing_parent() {
        let temp = TempDir::new().unwrap();
        let src_dir = temp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        // File doesn't exist but parent does
        let result = validate_path_is_internal(temp.path(), "src/new_file.rs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_deep_path_traversal_blocked() {
        let temp = TempDir::new().unwrap();
        let src_dir = temp.path().join("src").join("deep");
        std::fs::create_dir_all(&src_dir).unwrap();

        let result = validate_path_is_internal(temp.path(), "src/deep/../../../outside.rs");
        assert!(result.is_err());
    }
}
