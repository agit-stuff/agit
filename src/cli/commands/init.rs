//! Implementation of the `agit init` command.

use std::fs;
use std::path::Path;

use crate::cli::args::InitArgs;
use crate::cli::commands::hooks;
use crate::error::{AgitError, Result};
use crate::storage::{FileHeadStore, FileIndexStore};
use crate::templates::{
    generate_versioned_protocol, AGIT_VERSION, TEMPLATE_FILES,
};

/// The AGIT directory name.
const AGIT_DIR: &str = ".agit";

/// Entries to add to .gitignore for AGIT (V2 Git-native storage).
/// In V2, all .agit/ content is local since shared data lives in Git refs.
const GITIGNORE_ENTRIES_V2: &str = r#"
# AGIT - AI-Native Git Wrapper (V2 Git-native storage)
# All local state - shared data is in refs/agit/* and Git ODB
.agit/

# MCP configs (shared with team)
!.mcp.json
!.cursor/mcp.json
!.vscode/mcp.json
"#;

/// Execute the `init` command.
pub fn execute(args: InitArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(AGIT_DIR);

    // Check if this is a git repository
    if !cwd.join(".git").exists() {
        return Err(AgitError::NotGitRepository);
    }

    // Handle --update mode: only update template files, don't reinitialize
    if args.update {
        if !agit_dir.exists() {
            println!("⚠️  AGIT is not initialized. Run `agit init` first.");
            return Ok(());
        }

        let updated = update_template_files(&cwd)?;
        if !updated {
            println!("No template files were updated. Ensure CLAUDE.md or .cursorrules exists.");
        }
        return Ok(());
    }

    // Check if already initialized (for normal init, not update mode)
    if agit_dir.exists() && !args.force {
        return Err(AgitError::AlreadyInitialized { path: agit_dir });
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

    // Install git hooks for automatic sync
    if !args.no_hooks {
        if let Err(e) = hooks::install_all_hooks(&cwd) {
            eprintln!("Warning: Failed to install git hooks: {}", e);
            eprintln!("You can install them manually with: agit hooks install");
        }
    }

    println!("\nInitialized AGIT repository in {}", agit_dir.display());

    if !args.no_templates {
        println!("\nGenerated instruction files:");
        for (name, _) in TEMPLATE_FILES {
            println!("  - {}", name);
        }
    }

    if !args.no_hooks {
        println!("\nInstalled git hooks: post-commit, post-checkout, post-merge, post-rewrite");
    }

    println!("\nAGIT is ready! MCP configs auto-detected by Cursor and Claude Code.");
    println!("Git hooks will keep agit in sync when you use native git commands.");
    println!("Restart your AI assistant to activate AGIT memory.");

    Ok(())
}

/// Create the `.agit` directory structure for V2 (Git-native) storage.
///
/// V2 storage uses Git's ODB for objects and refs/agit/* for branch refs,
/// so we only need local state files in .agit/:
/// - HEAD: Current branch pointer (local)
/// - index: Staged trace entries (local)
/// - config.json: Local configuration
/// - tmp/: Temporary files
fn create_agit_structure(agit_dir: &Path) -> Result<()> {
    // Create main directory and tmp
    fs::create_dir_all(agit_dir)?;
    fs::create_dir_all(agit_dir.join("tmp"))?;

    // Create config.json with storage version
    let config_path = agit_dir.join("config.json");
    if !config_path.exists() {
        fs::write(&config_path, "{\"storage_version\": 2}\n")?;
    }

    // Initialize HEAD to main (local state only)
    let head_store = FileHeadStore::new(agit_dir);
    head_store.ensure_exists("main")?;

    // Initialize empty index (local state only)
    let index_store = FileIndexStore::new(agit_dir);
    index_store.ensure_exists()?;

    // Note: For V2, we don't create objects/ or refs/heads/ directories
    // Objects are stored in Git ODB, refs are stored in refs/agit/heads/*

    Ok(())
}

/// Marker to detect if AGIT policy is already present.
const AGIT_POLICY_MARKER: &str = "# SYSTEM POLICY: AGIT MEMORY";

/// Start marker for the system protocol block (matches both versioned and unversioned).
const PROTOCOL_START_MARKER: &str = "<system_protocol";

/// End marker for the system protocol block.
const PROTOCOL_END_MARKER: &str = "</system_protocol>";

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

/// Update AI instruction template files with the latest system protocol.
///
/// This function finds and replaces the `<system_protocol>...</system_protocol>` block
/// in existing files, preserving any user-defined content outside the markers.
///
/// Returns `Ok(true)` if at least one file was updated, `Ok(false)` if no files
/// were found or had the markers.
fn update_template_files(project_dir: &Path) -> Result<bool> {
    let versioned_protocol = generate_versioned_protocol();
    let mut any_updated = false;

    // Template files to update
    let template_files = ["CLAUDE.md", ".cursorrules"];

    for filename in template_files {
        let path = project_dir.join(filename);

        if !path.exists() {
            continue;
        }

        let existing = fs::read_to_string(&path)?;

        // Find the protocol block markers
        let start_pos = match existing.find(PROTOCOL_START_MARKER) {
            Some(pos) => pos,
            None => {
                println!(
                    "⚠️  Could not find Agit block in {}. Please run `agit init --force` to reset the file completely.",
                    filename
                );
                continue;
            }
        };

        // Find the closing tag (search from start_pos to avoid false matches)
        let end_pos = match existing[start_pos..].find(PROTOCOL_END_MARKER) {
            Some(pos) => start_pos + pos + PROTOCOL_END_MARKER.len(),
            None => {
                println!(
                    "⚠️  Could not find closing </system_protocol> in {}. Please run `agit init --force` to reset the file completely.",
                    filename
                );
                continue;
            }
        };

        // Build new content: before + versioned protocol + after
        let before = &existing[..start_pos];
        let after = &existing[end_pos..];
        let new_content = format!("{}{}{}", before, versioned_protocol, after);

        fs::write(&path, new_content)?;
        println!("✅ Updated AI Protocols in {} to v{}", filename, AGIT_VERSION);
        any_updated = true;
    }

    Ok(any_updated)
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
/// Uses V2 (Git-native) entries by default.
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

    // Append AGIT V2 entries (Git-native storage)
    let new_content = if existing.ends_with('\n') || existing.is_empty() {
        format!("{}{}", existing, GITIGNORE_ENTRIES_V2)
    } else {
        format!("{}\n{}", existing, GITIGNORE_ENTRIES_V2)
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

        // V2 structure: only local state files, no objects/ or refs/
        assert!(agit_dir.exists());
        assert!(agit_dir.join("tmp").exists());
        assert!(agit_dir.join("config.json").exists());
        assert!(agit_dir.join("HEAD").exists());
        assert!(agit_dir.join("index").exists());

        // V2: objects and refs are NOT in .agit/ (they're in Git ODB and refs/agit/*)
        assert!(!agit_dir.join("objects").exists());
        assert!(!agit_dir.join("refs").exists());

        // Verify config has storage version
        let config = fs::read_to_string(agit_dir.join("config.json")).unwrap();
        assert!(config.contains("\"storage_version\": 2"));
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
        // V2: entire .agit/ is ignored (local state only)
        assert!(content.contains(".agit/"));
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

    #[test]
    fn test_update_template_files_replaces_protocol_block() {
        use crate::templates::AGIT_VERSION;

        let temp = setup_git_repo();

        // Create a CLAUDE.md with old (unversioned) protocol and custom content
        let old_content = r#"# My Custom Rules

Some custom instructions here.

# SYSTEM POLICY: AGIT MEMORY

<system_protocol>

  <critical_rule id="OLD_RULE">
    <instruction>Old instruction content</instruction>
  </critical_rule>

</system_protocol>

# More Custom Rules

Additional custom content below.
"#;
        fs::write(temp.path().join("CLAUDE.md"), old_content).unwrap();

        // Run the update
        let updated = update_template_files(temp.path()).unwrap();
        assert!(updated, "Should have updated at least one file");

        // Verify the result
        let new_content = fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap();

        // Custom content should be preserved
        assert!(new_content.contains("# My Custom Rules"));
        assert!(new_content.contains("Some custom instructions here."));
        assert!(new_content.contains("# More Custom Rules"));
        assert!(new_content.contains("Additional custom content below."));

        // Old protocol content should be replaced
        assert!(!new_content.contains("OLD_RULE"));
        assert!(!new_content.contains("Old instruction content"));

        // New versioned protocol should be present
        assert!(new_content.contains(&format!("<system_protocol version=\"{}\">", AGIT_VERSION)));
        assert!(new_content.contains("BATCH_LOGGING"));
        assert!(new_content.contains("</system_protocol>"));
    }

    #[test]
    fn test_update_template_files_handles_versioned_protocol() {
        use crate::templates::AGIT_VERSION;

        let temp = setup_git_repo();

        // Create a CLAUDE.md with already versioned protocol
        let old_content = r#"# SYSTEM POLICY: AGIT MEMORY

<system_protocol version="0.0.1">

  <critical_rule id="OLD_VERSIONED_RULE">
    <instruction>Some old versioned instruction</instruction>
  </critical_rule>

</system_protocol>
"#;
        fs::write(temp.path().join("CLAUDE.md"), old_content).unwrap();

        // Run the update
        let updated = update_template_files(temp.path()).unwrap();
        assert!(updated);

        // Verify the version was updated
        let new_content = fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap();
        assert!(!new_content.contains("version=\"0.0.1\""));
        assert!(new_content.contains(&format!("version=\"{}\"", AGIT_VERSION)));
        assert!(!new_content.contains("OLD_VERSIONED_RULE"));
    }

    #[test]
    fn test_update_template_files_no_marker_returns_false() {
        let temp = setup_git_repo();

        // Create a CLAUDE.md without any protocol block
        let content = "# My Project\n\nJust some regular markdown content.\n";
        fs::write(temp.path().join("CLAUDE.md"), content).unwrap();

        // Run the update - should not update anything
        let updated = update_template_files(temp.path()).unwrap();
        assert!(!updated, "Should return false when no protocol block found");

        // Content should remain unchanged
        let after_content = fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap();
        assert_eq!(content, after_content);
    }

    #[test]
    fn test_update_template_files_no_files_returns_false() {
        let temp = setup_git_repo();

        // No template files exist
        let updated = update_template_files(temp.path()).unwrap();
        assert!(!updated, "Should return false when no template files exist");
    }

    #[test]
    fn test_update_template_files_updates_cursorrules() {
        use crate::templates::AGIT_VERSION;

        let temp = setup_git_repo();

        // Create a .cursorrules with old protocol
        let old_content = r#"# SYSTEM POLICY: AGIT MEMORY

<system_protocol>

  <critical_rule id="CURSOR_OLD">
    <instruction>Old cursor rule</instruction>
  </critical_rule>

</system_protocol>
"#;
        fs::write(temp.path().join(".cursorrules"), old_content).unwrap();

        // Run the update
        let updated = update_template_files(temp.path()).unwrap();
        assert!(updated);

        // Verify .cursorrules was updated
        let new_content = fs::read_to_string(temp.path().join(".cursorrules")).unwrap();
        assert!(new_content.contains(&format!("<system_protocol version=\"{}\">", AGIT_VERSION)));
        assert!(!new_content.contains("CURSOR_OLD"));
        assert!(new_content.contains("BATCH_LOGGING"));
    }
}
