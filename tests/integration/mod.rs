//! Integration test utilities for AGIT.
//!
//! This module provides shared test helpers and fixtures.

use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

/// Create a test git repository with initial commit.
pub fn create_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();

    // Initialize git
    Command::new("git")
        .args(["init"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to init git repo");

    // Configure git user
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to configure git email");

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to configure git name");

    // Create initial commit
    fs::write(temp.path().join("README.md"), "# Test Project").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(temp.path())
        .output()
        .expect("Failed to stage files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to create initial commit");

    temp
}

/// Initialize AGIT in a test repository.
pub fn init_agit(repo_path: &Path) {
    use assert_cmd::Command;

    Command::cargo_bin("agit")
        .unwrap()
        .arg("init")
        .current_dir(repo_path)
        .assert()
        .success();
}
