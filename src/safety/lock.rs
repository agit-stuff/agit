//! File locking for concurrent access protection.
//!
//! Uses `fs2` for cross-platform advisory file locking.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use fs2::FileExt;

use crate::error::{AgitError, Result};

/// A file-based lock guard.
///
/// The lock is automatically released when this guard is dropped.
pub struct LockGuard {
    _file: File,
    path: PathBuf,
}

impl LockGuard {
    /// Acquire an exclusive lock on the given path.
    ///
    /// This will block until the lock is acquired.
    pub fn acquire(lock_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(lock_path)
            .map_err(|e| AgitError::LockFailed {
                reason: format!("Failed to open lock file: {}", e),
            })?;

        file.lock_exclusive().map_err(|e| AgitError::LockFailed {
            reason: format!("Failed to acquire lock: {}", e),
        })?;

        Ok(Self {
            _file: file,
            path: lock_path.to_path_buf(),
        })
    }

    /// Try to acquire an exclusive lock without blocking.
    ///
    /// Returns `None` if the lock is already held.
    pub fn try_acquire(lock_path: &Path) -> Result<Option<Self>> {
        // Ensure parent directory exists
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(lock_path)
            .map_err(|e| AgitError::LockFailed {
                reason: format!("Failed to open lock file: {}", e),
            })?;

        match file.try_lock_exclusive() {
            Ok(()) => Ok(Some(Self {
                _file: file,
                path: lock_path.to_path_buf(),
            })),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(AgitError::LockFailed {
                reason: format!("Failed to acquire lock: {}", e),
            }),
        }
    }

    /// Get the path to the lock file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Lock is automatically released when the file is closed
        // We could also explicitly unlock here if needed
    }
}

/// Convenience function to get the default lock path for an AGIT directory.
pub fn lock_path(agit_dir: &Path) -> PathBuf {
    agit_dir.join("LOCK")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_lock_acquire_and_release() {
        let temp = TempDir::new().unwrap();
        let lock_file = temp.path().join("test.lock");

        // Acquire lock
        let guard = LockGuard::acquire(&lock_file).unwrap();
        assert!(lock_file.exists());

        // Drop guard to release
        drop(guard);
    }

    #[test]
    fn test_try_acquire_when_free() {
        let temp = TempDir::new().unwrap();
        let lock_file = temp.path().join("test.lock");

        let guard = LockGuard::try_acquire(&lock_file).unwrap();
        assert!(guard.is_some());
    }

    #[test]
    fn test_lock_path() {
        let agit_dir = Path::new("/project/.agit");
        let path = lock_path(agit_dir);
        assert_eq!(path, PathBuf::from("/project/.agit/LOCK"));
    }
}
