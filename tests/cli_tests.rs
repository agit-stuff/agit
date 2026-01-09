//! Integration tests for AGIT CLI commands.
//!
//! These tests verify the end-to-end behavior of the CLI commands.

use std::fs;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

/// Helper to create a test git repository.
fn create_test_repo() -> TempDir {
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

/// Get a command for the agit binary.
fn agit_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_agit"))
}

#[test]
fn test_init_creates_agit_directory() {
    let temp = create_test_repo();

    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized AGIT"));

    // Verify .agit directory structure
    let agit_dir = temp.path().join(".agit");
    assert!(agit_dir.exists());
    assert!(agit_dir.join("objects").is_dir());
    assert!(agit_dir.join("refs/heads").is_dir());
    assert!(agit_dir.join("HEAD").exists());
    assert!(agit_dir.join("index").exists());
}

#[test]
fn test_init_creates_instruction_files() {
    let temp = create_test_repo();

    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // Verify instruction files were created
    assert!(temp.path().join("CLAUDE.md").exists());
    assert!(temp.path().join(".cursorrules").exists());

    // Verify MCP config files were created
    assert!(temp.path().join(".mcp.json").exists());
    assert!(temp.path().join(".cursor/mcp.json").exists());
}

#[test]
fn test_init_fails_if_not_git_repo() {
    let temp = TempDir::new().unwrap();

    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .failure();
}

#[test]
fn test_init_fails_if_already_initialized() {
    let temp = create_test_repo();

    // First init
    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // Second init should fail
    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already initialized"));
}

#[test]
fn test_record_adds_entry_to_index() {
    let temp = create_test_repo();

    // Initialize
    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // Record a thought
    agit_cmd()
        .args(["record", "Planning to refactor the auth module"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded"));

    // Verify entry in index
    let index_content = fs::read_to_string(temp.path().join(".agit/index")).unwrap();
    assert!(index_content.contains("Planning to refactor"));
}

#[test]
fn test_record_fails_if_not_initialized() {
    let temp = create_test_repo();

    agit_cmd()
        .args(["record", "Some thought"])
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not initialized"));
}

#[test]
fn test_status_shows_pending_thoughts() {
    let temp = create_test_repo();

    // Initialize
    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // Check status with no thoughts
    agit_cmd()
        .arg("status")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No pending thoughts"));

    // Record a thought
    agit_cmd()
        .args(["record", "Test thought"])
        .current_dir(temp.path())
        .assert()
        .success();

    // Check status with pending thought
    agit_cmd()
        .arg("status")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Pending thoughts: 1"));
}

#[test]
fn test_commit_creates_neural_commit() {
    let temp = create_test_repo();

    // Initialize
    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // Record thoughts
    agit_cmd()
        .args(["record", "User wants to add auth"])
        .current_dir(temp.path())
        .assert()
        .success();

    // Create neural commit
    agit_cmd()
        .args(["commit", "-m", "Add authentication"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created neural commit"));

    // Verify objects were created
    let objects_dir = temp.path().join(".agit/objects");
    let object_count = fs::read_dir(&objects_dir)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_type().unwrap().is_dir())
        .count();
    assert!(object_count > 0);
}

#[test]
fn test_commit_clears_index() {
    let temp = create_test_repo();

    // Initialize and record
    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    agit_cmd()
        .args(["record", "Test thought"])
        .current_dir(temp.path())
        .assert()
        .success();

    // Commit
    agit_cmd()
        .args(["commit", "-m", "Test commit"])
        .current_dir(temp.path())
        .assert()
        .success();

    // Index should be empty now
    let index_content = fs::read_to_string(temp.path().join(".agit/index")).unwrap();
    assert!(index_content.trim().is_empty());
}

#[test]
fn test_log_shows_commits() {
    let temp = create_test_repo();

    // Initialize and create a commit
    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    agit_cmd()
        .args(["record", "First thought"])
        .current_dir(temp.path())
        .assert()
        .success();

    agit_cmd()
        .args(["commit", "-m", "First neural commit"])
        .current_dir(temp.path())
        .assert()
        .success();

    // Check log
    agit_cmd()
        .arg("log")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("commit"));
}

#[test]
fn test_log_empty_repo() {
    let temp = create_test_repo();

    // Initialize but don't commit
    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // Log should indicate no commits
    agit_cmd()
        .arg("log")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No neural commits"));
}

#[test]
fn test_show_displays_commit_details() {
    let temp = create_test_repo();

    // Initialize and create a commit
    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    agit_cmd()
        .args(["record", "Add feature X"])
        .current_dir(temp.path())
        .assert()
        .success();

    agit_cmd()
        .args(["commit", "-m", "Add feature X"])
        .current_dir(temp.path())
        .assert()
        .success();

    // Show HEAD
    agit_cmd()
        .arg("show")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Neural Commit"))
        .stdout(predicate::str::contains("Summary"));
}

#[test]
fn test_full_workflow() {
    let temp = create_test_repo();

    // 1. Initialize
    agit_cmd()
        .arg("init")
        .current_dir(temp.path())
        .assert()
        .success();

    // 2. Record user intent (using --intent flag)
    agit_cmd()
        .args(["record", "--intent", "Add user authentication"])
        .current_dir(temp.path())
        .assert()
        .success();

    // 3. Record AI reasoning (using --ai flag)
    agit_cmd()
        .args(["record", "--ai", "Will implement JWT-based auth"])
        .current_dir(temp.path())
        .assert()
        .success();

    // 4. Make some code changes (simulated)
    fs::write(temp.path().join("auth.rs"), "// Auth module").unwrap();

    // 5. Create git commit
    Command::new("git")
        .args(["add", "."])
        .current_dir(temp.path())
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Add auth module"])
        .current_dir(temp.path())
        .output()
        .unwrap();

    // 6. Create neural commit
    agit_cmd()
        .args(["commit", "-m", "Add user authentication"])
        .current_dir(temp.path())
        .assert()
        .success();

    // 7. Verify log shows the summary
    agit_cmd()
        .arg("log")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Intent"));

    // 8. Verify show displays full context
    agit_cmd()
        .args(["show", "--trace"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Trace"));
}
