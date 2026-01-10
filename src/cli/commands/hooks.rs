//! Implementation of the `agit hooks` command.
//!
//! Manages git hook installation for automatic agit sync.

use std::fs;
use std::path::Path;

use crate::cli::args::{HooksArgs, HooksCommands};
use crate::error::{AgitError, Result};

/// Marker comments for agit hook sections.
const HOOK_START_MARKER: &str = "# AGIT-HOOK-START";
const HOOK_END_MARKER: &str = "# AGIT-HOOK-END";

/// The hooks we install.
const HOOKS: &[(&str, &str)] = &[
    ("post-commit", "post-commit"),
    ("post-checkout", "post-checkout"),
    ("post-merge", "post-merge"),
    ("post-rewrite", "post-rewrite"),
];

/// Execute the `hooks` command.
pub fn execute(args: HooksArgs) -> Result<()> {
    match args.command {
        HooksCommands::Install => install_hooks_command(),
        HooksCommands::Uninstall => uninstall_hooks_command(),
        HooksCommands::Status => status_hooks_command(),
    }
}

/// Install all agit git hooks.
fn install_hooks_command() -> Result<()> {
    let cwd = std::env::current_dir()?;
    install_all_hooks(&cwd)?;
    println!("Installed agit git hooks:");
    for (name, _) in HOOKS {
        println!("  - {}", name);
    }
    println!("\nAgit will now automatically sync when you use git commands.");
    Ok(())
}

/// Uninstall all agit git hooks.
fn uninstall_hooks_command() -> Result<()> {
    let cwd = std::env::current_dir()?;
    uninstall_all_hooks(&cwd)?;
    println!("Removed agit git hooks.");
    Ok(())
}

/// Show hook installation status.
fn status_hooks_command() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let hooks_dir = cwd.join(".git").join("hooks");

    if !hooks_dir.exists() {
        println!("Not a git repository (no .git/hooks directory).");
        return Ok(());
    }

    println!("Agit hook status:");
    for (name, _) in HOOKS {
        let hook_path = hooks_dir.join(name);
        let status = if hook_path.exists() {
            let content = fs::read_to_string(&hook_path).unwrap_or_default();
            if content.contains(HOOK_START_MARKER) {
                "installed"
            } else {
                "exists (not agit)"
            }
        } else {
            "not installed"
        };
        println!("  {}: {}", name, status);
    }

    Ok(())
}

/// Install all agit hooks in the given project directory.
pub fn install_all_hooks(project_dir: &Path) -> Result<()> {
    let hooks_dir = project_dir.join(".git").join("hooks");

    if !hooks_dir.exists() {
        return Err(AgitError::NotGitRepository);
    }

    // Ensure hooks directory exists
    fs::create_dir_all(&hooks_dir)?;

    for (hook_name, hook_type) in HOOKS {
        install_hook(&hooks_dir, hook_name, hook_type)?;
    }

    Ok(())
}

/// Uninstall all agit hooks from the given project directory.
pub fn uninstall_all_hooks(project_dir: &Path) -> Result<()> {
    let hooks_dir = project_dir.join(".git").join("hooks");

    if !hooks_dir.exists() {
        return Ok(());
    }

    for (hook_name, _) in HOOKS {
        uninstall_hook(&hooks_dir, hook_name)?;
    }

    Ok(())
}

/// Install a single hook.
fn install_hook(hooks_dir: &Path, hook_name: &str, hook_type: &str) -> Result<()> {
    let hook_path = hooks_dir.join(hook_name);
    let hook_script = generate_hook_script(hook_type);

    if hook_path.exists() {
        // Read existing content
        let existing = fs::read_to_string(&hook_path)?;

        // Check if agit section already exists
        if existing.contains(HOOK_START_MARKER) {
            // Already installed, update the section
            let new_content = replace_agit_section(&existing, &hook_script);
            fs::write(&hook_path, new_content)?;
        } else {
            // Append agit section to existing hook
            let new_content = format!("{}\n\n{}", existing.trim_end(), hook_script);
            fs::write(&hook_path, new_content)?;
        }
    } else {
        // Create new hook file
        let content = format!("#!/bin/sh\n\n{}\n", hook_script);
        fs::write(&hook_path, content)?;
    }

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)?;
    }

    Ok(())
}

/// Uninstall a single hook.
fn uninstall_hook(hooks_dir: &Path, hook_name: &str) -> Result<()> {
    let hook_path = hooks_dir.join(hook_name);

    if !hook_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&hook_path)?;

    if !content.contains(HOOK_START_MARKER) {
        // No agit section, nothing to do
        return Ok(());
    }

    // Remove agit section
    let new_content = remove_agit_section(&content);

    // Check if file is now effectively empty (just shebang and whitespace)
    let trimmed = new_content
        .lines()
        .filter(|l| !l.starts_with("#!") && !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if trimmed.trim().is_empty() {
        // File is empty, remove it
        fs::remove_file(&hook_path)?;
    } else {
        // Write updated content
        fs::write(&hook_path, new_content)?;
    }

    Ok(())
}

/// Generate the hook script for a given hook type.
fn generate_hook_script(hook_type: &str) -> String {
    format!(
        r#"{HOOK_START_MARKER}
# Automatically sync agit state with git
# Do not edit this section - managed by agit
if command -v agit >/dev/null 2>&1; then
    agit sync --hook {hook_type} --quiet 2>/dev/null || true
fi
{HOOK_END_MARKER}"#,
        HOOK_START_MARKER = HOOK_START_MARKER,
        hook_type = hook_type,
        HOOK_END_MARKER = HOOK_END_MARKER
    )
}

/// Replace the agit section in existing content.
fn replace_agit_section(content: &str, new_section: &str) -> String {
    let mut result = String::new();
    let mut in_agit_section = false;
    let mut section_replaced = false;

    for line in content.lines() {
        if line.contains(HOOK_START_MARKER) {
            in_agit_section = true;
            if !section_replaced {
                result.push_str(new_section);
                result.push('\n');
                section_replaced = true;
            }
        } else if line.contains(HOOK_END_MARKER) {
            in_agit_section = false;
        } else if !in_agit_section {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

/// Remove the agit section from content.
fn remove_agit_section(content: &str) -> String {
    let mut result = String::new();
    let mut in_agit_section = false;

    for line in content.lines() {
        if line.contains(HOOK_START_MARKER) {
            in_agit_section = true;
        } else if line.contains(HOOK_END_MARKER) {
            in_agit_section = false;
        } else if !in_agit_section {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Clean up extra blank lines
    while result.ends_with("\n\n") {
        result.pop();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_git_repo() -> TempDir {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git/hooks")).unwrap();
        temp
    }

    #[test]
    fn test_install_creates_new_hook() {
        let temp = setup_git_repo();
        let hooks_dir = temp.path().join(".git/hooks");

        install_hook(&hooks_dir, "post-commit", "post-commit").unwrap();

        let hook_path = hooks_dir.join("post-commit");
        assert!(hook_path.exists());

        let content = fs::read_to_string(&hook_path).unwrap();
        assert!(content.contains("#!/bin/sh"));
        assert!(content.contains(HOOK_START_MARKER));
        assert!(content.contains("agit sync --hook post-commit"));
        assert!(content.contains(HOOK_END_MARKER));
    }

    #[test]
    fn test_install_appends_to_existing_hook() {
        let temp = setup_git_repo();
        let hooks_dir = temp.path().join(".git/hooks");
        let hook_path = hooks_dir.join("post-commit");

        // Create existing hook
        fs::write(&hook_path, "#!/bin/sh\necho 'existing hook'\n").unwrap();

        install_hook(&hooks_dir, "post-commit", "post-commit").unwrap();

        let content = fs::read_to_string(&hook_path).unwrap();
        assert!(content.contains("echo 'existing hook'"));
        assert!(content.contains(HOOK_START_MARKER));
        assert!(content.contains("agit sync"));
    }

    #[test]
    fn test_install_is_idempotent() {
        let temp = setup_git_repo();
        let hooks_dir = temp.path().join(".git/hooks");

        install_hook(&hooks_dir, "post-commit", "post-commit").unwrap();
        let content1 = fs::read_to_string(hooks_dir.join("post-commit")).unwrap();

        install_hook(&hooks_dir, "post-commit", "post-commit").unwrap();
        let content2 = fs::read_to_string(hooks_dir.join("post-commit")).unwrap();

        // Should only have one agit section
        assert_eq!(
            content1.matches(HOOK_START_MARKER).count(),
            1,
            "Should have exactly one start marker"
        );
        assert_eq!(
            content2.matches(HOOK_START_MARKER).count(),
            1,
            "Should still have exactly one start marker after reinstall"
        );
    }

    #[test]
    fn test_uninstall_removes_agit_section() {
        let temp = setup_git_repo();
        let hooks_dir = temp.path().join(".git/hooks");
        let hook_path = hooks_dir.join("post-commit");

        // Create hook with agit and custom content
        let content = format!(
            "#!/bin/sh\necho 'custom'\n\n{}\n",
            generate_hook_script("post-commit")
        );
        fs::write(&hook_path, content).unwrap();

        uninstall_hook(&hooks_dir, "post-commit").unwrap();

        let result = fs::read_to_string(&hook_path).unwrap();
        assert!(result.contains("echo 'custom'"));
        assert!(!result.contains(HOOK_START_MARKER));
        assert!(!result.contains("agit sync"));
    }

    #[test]
    fn test_uninstall_removes_empty_hook() {
        let temp = setup_git_repo();
        let hooks_dir = temp.path().join(".git/hooks");
        let hook_path = hooks_dir.join("post-commit");

        // Create hook with only agit content
        let content = format!("#!/bin/sh\n\n{}\n", generate_hook_script("post-commit"));
        fs::write(&hook_path, content).unwrap();

        uninstall_hook(&hooks_dir, "post-commit").unwrap();

        // Hook file should be removed since it's now empty
        assert!(!hook_path.exists());
    }

    #[test]
    fn test_generate_hook_script() {
        let script = generate_hook_script("post-commit");
        assert!(script.contains(HOOK_START_MARKER));
        assert!(script.contains("agit sync --hook post-commit --quiet"));
        assert!(script.contains(HOOK_END_MARKER));
        assert!(script.contains("|| true")); // Fail silently
    }
}
