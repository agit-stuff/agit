//! Implementation of the `agit init` command.

use std::fs;
use std::path::Path;

use crate::cli::args::InitArgs;
use crate::error::{AgitError, Result};
use crate::storage::{FileHeadStore, FileIndexStore, FileRefStore};
use crate::templates::TEMPLATE_FILES;

/// The AGIT directory name.
const AGIT_DIR: &str = ".agit";

/// Entries to add to .gitignore for AGIT.
const GITIGNORE_ENTRIES: &str = r#"
# AGIT - AI-Native Git Wrapper
# Local state (not shared)
.agit/config.json
.agit/HEAD
.agit/index
.agit/LOCK
.agit/tmp/

# Shared data (tracked)
!/.agit/objects/
!/.agit/refs/
"#;

/// Execute the `init` command.
pub fn execute(args: InitArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(AGIT_DIR);

    // Check if already initialized
    if agit_dir.exists() && !args.force {
        return Err(AgitError::AlreadyInitialized { path: agit_dir });
    }

    // Check if this is a git repository
    if !cwd.join(".git").exists() {
        return Err(AgitError::NotGitRepository);
    }

    // Create .agit directory structure
    create_agit_structure(&agit_dir)?;

    // Generate AI instruction files
    if !args.no_templates {
        generate_template_files(&cwd)?;
    }

    // Update .gitignore
    if !args.no_gitignore {
        update_gitignore(&cwd)?;
    }

    println!("Initialized AGIT repository in {}", agit_dir.display());

    if !args.no_templates {
        println!("\nGenerated instruction files:");
        for (name, _) in TEMPLATE_FILES {
            println!("  - {}", name);
        }
    }

    println!("\nAGIT is ready for Cursor, Claude Code, and Windsurf.");
    println!("Start your AI assistant and it will automatically log thoughts to AGIT.");

    Ok(())
}

/// Create the `.agit` directory structure.
fn create_agit_structure(agit_dir: &Path) -> Result<()> {
    // Create main directories
    fs::create_dir_all(agit_dir)?;
    fs::create_dir_all(agit_dir.join("objects"))?;
    fs::create_dir_all(agit_dir.join("refs").join("heads"))?;
    fs::create_dir_all(agit_dir.join("tmp"))?;

    // Create empty config.json
    let config_path = agit_dir.join("config.json");
    if !config_path.exists() {
        fs::write(&config_path, "{}\n")?;
    }

    // Initialize HEAD to main
    let head_store = FileHeadStore::new(agit_dir);
    head_store.ensure_exists("main")?;

    // Initialize empty index
    let index_store = FileIndexStore::new(agit_dir);
    index_store.ensure_exists()?;

    // Initialize refs directory
    let ref_store = FileRefStore::new(agit_dir);
    ref_store.ensure_exists()?;

    Ok(())
}

/// Generate AI instruction template files.
fn generate_template_files(project_dir: &Path) -> Result<()> {
    for (filename, content) in TEMPLATE_FILES {
        let path = project_dir.join(filename);

        // Don't overwrite existing files
        if path.exists() {
            println!("Skipping {} (already exists)", filename);
            continue;
        }

        fs::write(&path, content)?;
    }

    Ok(())
}

/// Update .gitignore with AGIT entries.
fn update_gitignore(project_dir: &Path) -> Result<()> {
    let gitignore_path = project_dir.join(".gitignore");

    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    // Check if AGIT entries already exist
    if existing.contains("# AGIT - AI-Native Git Wrapper") {
        return Ok(());
    }

    // Append AGIT entries
    let new_content = if existing.ends_with('\n') || existing.is_empty() {
        format!("{}{}", existing, GITIGNORE_ENTRIES)
    } else {
        format!("{}\n{}", existing, GITIGNORE_ENTRIES)
    };

    fs::write(&gitignore_path, new_content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_git_repo() -> TempDir {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join(".git")).unwrap();
        temp
    }

    #[test]
    fn test_create_agit_structure() {
        let temp = setup_git_repo();
        let agit_dir = temp.path().join(".agit");

        create_agit_structure(&agit_dir).unwrap();

        assert!(agit_dir.exists());
        assert!(agit_dir.join("objects").exists());
        assert!(agit_dir.join("refs/heads").exists());
        assert!(agit_dir.join("tmp").exists());
        assert!(agit_dir.join("config.json").exists());
        assert!(agit_dir.join("HEAD").exists());
        assert!(agit_dir.join("index").exists());
    }

    #[test]
    fn test_generate_template_files() {
        let temp = setup_git_repo();

        generate_template_files(temp.path()).unwrap();

        assert!(temp.path().join("CLAUDE.md").exists());
        assert!(temp.path().join(".cursorrules").exists());
        assert!(temp.path().join(".windsurfrules").exists());
    }

    #[test]
    fn test_update_gitignore_creates_new() {
        let temp = setup_git_repo();

        update_gitignore(temp.path()).unwrap();

        let content = fs::read_to_string(temp.path().join(".gitignore")).unwrap();
        assert!(content.contains("# AGIT - AI-Native Git Wrapper"));
        assert!(content.contains(".agit/config.json"));
    }

    #[test]
    fn test_update_gitignore_appends() {
        let temp = setup_git_repo();
        fs::write(temp.path().join(".gitignore"), "node_modules/\n").unwrap();

        update_gitignore(temp.path()).unwrap();

        let content = fs::read_to_string(temp.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains("# AGIT - AI-Native Git Wrapper"));
    }

    #[test]
    fn test_update_gitignore_idempotent() {
        let temp = setup_git_repo();

        update_gitignore(temp.path()).unwrap();
        let content1 = fs::read_to_string(temp.path().join(".gitignore")).unwrap();

        update_gitignore(temp.path()).unwrap();
        let content2 = fs::read_to_string(temp.path().join(".gitignore")).unwrap();

        assert_eq!(content1, content2);
    }
}
