//! Atomic file write operations.
//!
//! Uses the write-flush-rename pattern to ensure atomic updates:
//! 1. Write to a temporary file
//! 2. Flush and sync to disk
//! 3. Atomically rename to the target path

use std::fs;
use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;

use crate::error::{AgitError, Result};

/// Atomically write content to a file.
///
/// This ensures that the file is either fully written or not at all,
/// preventing partial writes that could corrupt data.
///
/// # Arguments
///
/// * `path` - The target file path
/// * `content` - The content to write
///
/// # Example
///
/// ```ignore
/// use agit::safety::atomic_write;
///
/// atomic_write(Path::new("config.json"), b"{}")?;
/// ```
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    // Get the directory for the temp file (same as target for atomic rename)
    let dir = path.parent().unwrap_or(Path::new("."));

    // Ensure the directory exists
    fs::create_dir_all(dir)?;

    // Create a temporary file in the same directory
    let mut temp_file = NamedTempFile::new_in(dir)?;

    // Write content
    temp_file.write_all(content)?;

    // Flush to OS buffer
    temp_file.flush()?;

    // Sync to disk (ensures durability)
    temp_file.as_file().sync_all()?;

    // Atomically rename to target path
    temp_file.persist(path).map_err(|e| AgitError::Io(e.error))?;

    Ok(())
}

/// Atomically write a string to a file.
///
/// Convenience wrapper around [`atomic_write`] for string content.
pub fn atomic_write_str(path: &Path, content: &str) -> Result<()> {
    atomic_write(path, content.as_bytes())
}

/// Atomically write JSON to a file.
///
/// Serializes the value as pretty-printed JSON and writes atomically.
pub fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    atomic_write_str(path, &json)
}

/// Read a file's contents, or return None if it doesn't exist.
pub fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(fs::read(path)?))
}

/// Read a file's contents as a string, or return None if it doesn't exist.
pub fn read_optional_string(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_atomic_write() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");

        atomic_write(&path, b"hello world").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_atomic_write_creates_parent_dirs() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("a").join("b").join("c").join("test.txt");

        atomic_write(&path, b"nested").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "nested");
    }

    #[test]
    fn test_atomic_write_overwrites() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");

        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "second");
    }

    #[test]
    fn test_atomic_write_json() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.json");

        #[derive(serde::Serialize)]
        struct Config {
            name: String,
            value: i32,
        }

        let config = Config {
            name: "test".to_string(),
            value: 42,
        };

        atomic_write_json(&path, &config).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"name\": \"test\""));
        assert!(content.contains("\"value\": 42"));
    }

    #[test]
    fn test_read_optional() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");

        // File doesn't exist
        assert!(read_optional(&path).unwrap().is_none());

        // Create file
        fs::write(&path, b"content").unwrap();
        assert_eq!(read_optional(&path).unwrap(), Some(b"content".to_vec()));
    }
}
