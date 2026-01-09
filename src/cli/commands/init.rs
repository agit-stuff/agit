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

# MCP configs (shared with team)
!.mcp.json
!.cursor/mcp.json
!.vscode/mcp.json
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

    // Generate MCP configuration files
    if !args.no_templates {
        println!("\nGenerated MCP configs:");
        generate_mcp_configs(&cwd)?;
    }

    // Update .gitignore
    if !args.no_gitignore {
        update_gitignore(&cwd)?;
    }

    println!("\nInitialized AGIT repository in {}", agit_dir.display());

    if !args.no_templates {
        println!("\nGenerated instruction files:");
        for (name, _) in TEMPLATE_FILES {
            println!("  - {}", name);
        }
    }

    println!("\nAGIT is ready! MCP configs auto-detected by Cursor and Claude Code.");
    println!("Restart your AI assistant to activate AGIT memory.");

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

/// Marker to detect if AGIT policy is already present.
const AGIT_POLICY_MARKER: &str = "# SYSTEM POLICY: AGIT MEMORY";

/// Generate AI instruction template files.
/// If the file exists, appends AGIT policy to preserve user content.
fn generate_template_files(project_dir: &Path) -> Result<()> {
    for (filename, content) in TEMPLATE_FILES {
        let path = project_dir.join(filename);

        if path.exists() {
            // Read existing content
            let existing = fs::read_to_string(&path)?;

            // Check if AGIT policy already exists
            if existing.contains(AGIT_POLICY_MARKER) {
                println!("Skipping {} (AGIT policy already present)", filename);
                continue;
            }

            // Append AGIT policy to existing content
            let new_content = if existing.ends_with('\n') {
                format!("{}\n{}", existing, content)
            } else {
                format!("{}\n\n{}", existing, content)
            };
            fs::write(&path, new_content)?;
            println!("Appended AGIT policy to existing {}", filename);
        } else {
            // Create new file
            fs::write(&path, content)?;
        }
    }

    Ok(())
}

/// Get the absolute path to the agit executable.
fn get_agit_command_path() -> String {
    // Try to get absolute path to current executable
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "agit".to_string()) // Fallback to PATH lookup
}

/// Generate the MCP config JSON content for Claude Code and Cursor.
fn generate_mcp_config(agit_path: &str) -> String {
    // Escape backslashes for JSON on Windows
    let escaped_path = agit_path.replace('\\', "\\\\");
    format!(
        r#"{{
  "mcpServers": {{
    "agit": {{
      "command": "{}",
      "args": ["server"]
    }}
  }}
}}
"#,
        escaped_path
    )
}

/// Generate the MCP config JSON content for VS Code Copilot.
/// VS Code uses "servers" key instead of "mcpServers".
fn generate_vscode_mcp_config(agit_path: &str) -> String {
    // Escape backslashes for JSON on Windows
    let escaped_path = agit_path.replace('\\', "\\\\");
    format!(
        r#"{{
  "servers": {{
    "agit": {{
      "command": "{}",
      "args": ["server"]
    }}
  }}
}}
"#,
        escaped_path
    )
}

/// Generate MCP configuration files for Claude Code, Cursor, and VS Code.
fn generate_mcp_configs(project_dir: &Path) -> Result<()> {
    let agit_path = get_agit_command_path();
    let mcp_config = generate_mcp_config(&agit_path);
    let vscode_config = generate_vscode_mcp_config(&agit_path);

    // Generate .mcp.json for Claude Code (project root)
    let mcp_json_path = project_dir.join(".mcp.json");
    if !mcp_json_path.exists() {
        fs::write(&mcp_json_path, &mcp_config)?;
        println!("  - .mcp.json (Claude Code)");
    } else {
        println!("  - Skipping .mcp.json (already exists)");
    }

    // Generate .cursor/mcp.json for Cursor
    let cursor_dir = project_dir.join(".cursor");
    fs::create_dir_all(&cursor_dir)?;
    let cursor_mcp_path = cursor_dir.join("mcp.json");
    if !cursor_mcp_path.exists() {
        fs::write(&cursor_mcp_path, &mcp_config)?;
        println!("  - .cursor/mcp.json (Cursor)");
    } else {
        println!("  - Skipping .cursor/mcp.json (already exists)");
    }

    // Generate .vscode/mcp.json for VS Code Copilot
    let vscode_dir = project_dir.join(".vscode");
    fs::create_dir_all(&vscode_dir)?;
    let vscode_mcp_path = vscode_dir.join("mcp.json");
    if !vscode_mcp_path.exists() {
        fs::write(&vscode_mcp_path, &vscode_config)?;
        println!("  - .vscode/mcp.json (VS Code Copilot)");
    } else {
        println!("  - Skipping .vscode/mcp.json (already exists)");
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
    }

    #[test]
    fn test_generate_template_files_appends_to_existing() {
        let temp = setup_git_repo();

        // Create existing CLAUDE.md with user content
        let user_content = "# My Project\n\nThis is my custom content.\n";
        fs::write(temp.path().join("CLAUDE.md"), user_content).unwrap();

        generate_template_files(temp.path()).unwrap();

        // Verify user content is preserved and AGIT policy is appended
        let content = fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap();
        assert!(content.contains("# My Project"));
        assert!(content.contains("This is my custom content"));
        assert!(content.contains(AGIT_POLICY_MARKER));
    }

    #[test]
    fn test_generate_template_files_skips_if_policy_exists() {
        let temp = setup_git_repo();

        // Create existing file with AGIT policy already present
        let existing = format!("# My Project\n\n{}\n", AGIT_POLICY_MARKER);
        fs::write(temp.path().join("CLAUDE.md"), &existing).unwrap();

        generate_template_files(temp.path()).unwrap();

        // Verify content wasn't duplicated
        let content = fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap();
        assert_eq!(
            content.matches(AGIT_POLICY_MARKER).count(),
            1,
            "AGIT policy should only appear once"
        );
    }

    #[test]
    fn test_generate_mcp_configs() {
        let temp = setup_git_repo();

        generate_mcp_configs(temp.path()).unwrap();

        assert!(temp.path().join(".mcp.json").exists());
        assert!(temp.path().join(".cursor/mcp.json").exists());
        assert!(temp.path().join(".vscode/mcp.json").exists());

        // Verify JSON structure for Claude Code/Cursor
        let mcp_content = fs::read_to_string(temp.path().join(".mcp.json")).unwrap();
        assert!(mcp_content.contains("mcpServers"));
        assert!(mcp_content.contains("agit"));
        assert!(mcp_content.contains("server"));

        // Verify VS Code uses "servers" key (not "mcpServers")
        let vscode_content = fs::read_to_string(temp.path().join(".vscode/mcp.json")).unwrap();
        assert!(vscode_content.contains("\"servers\""));
        assert!(!vscode_content.contains("mcpServers"));
        assert!(vscode_content.contains("agit"));
    }

    #[test]
    fn test_get_agit_command_path() {
        let path = get_agit_command_path();
        // Should either be an absolute path or "agit" fallback
        assert!(!path.is_empty());
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
