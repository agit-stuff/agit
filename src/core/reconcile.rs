//! Semantic conflict reconciliation for Agit.
//!
//! This module provides bidirectional reconciliation between Git and Agit:
//!
//! **Forward reconciliation (Ghost Commits)**:
//! Detects external git commits that happened while Agit had pending thoughts.
//! Checks for semantic conflicts where ghost commits modified files mentioned
//! in pending thoughts.
//!
//! **Backward reconciliation (Rewind/Dangling Head)**:
//! Detects when Git has "rewound" (e.g., `git reset --hard`) and Agit's HEAD
//! points to a neural commit attached to a git commit that no longer exists.
//! Snaps Agit back to a valid ancestor.

use crate::domain::{IndexEntry, WrappedNeuralCommit};
use crate::error::Result;
use crate::git::GitRepository;
use crate::storage::{ObjectStore, RefStore};

/// Information about ghost commits (external git commits Agit missed).
#[derive(Debug, Clone)]
pub struct GhostCommitInfo {
    /// The git commit hashes of external commits.
    pub git_hashes: Vec<String>,
    /// All files changed across all ghost commits.
    pub changed_files: Vec<String>,
}

/// Result of checking for semantic conflicts.
#[derive(Debug, Clone)]
pub struct ConflictCheckResult {
    /// Whether a semantic conflict was detected.
    pub has_conflict: bool,
    /// Files that overlap between ghost commits and pending thoughts.
    pub conflicting_files: Vec<String>,
    /// Information about the ghost commits (if any were detected).
    pub ghost_info: Option<GhostCommitInfo>,
}

impl ConflictCheckResult {
    /// Create a result indicating no conflicts.
    pub fn no_conflict() -> Self {
        Self {
            has_conflict: false,
            conflicting_files: Vec::new(),
            ghost_info: None,
        }
    }

    /// Create a result indicating a conflict.
    pub fn conflict(conflicting_files: Vec<String>, ghost_info: GhostCommitInfo) -> Self {
        Self {
            has_conflict: true,
            conflicting_files,
            ghost_info: Some(ghost_info),
        }
    }

    /// Create a result with ghost commits but no conflict.
    pub fn no_conflict_with_ghost(ghost_info: GhostCommitInfo) -> Self {
        Self {
            has_conflict: false,
            conflicting_files: Vec::new(),
            ghost_info: Some(ghost_info),
        }
    }
}

/// Detect ghost commits by comparing Git HEAD with the last known neural commit's git_hash.
///
/// Ghost commits are external git commits that happened while Agit had pending
/// thoughts. This can occur when:
/// - User ran `git commit` directly (bypassing Agit)
/// - User pulled changes from a remote
/// - A teammate pushed changes
///
/// # Arguments
///
/// * `git` - Git repository wrapper
/// * `objects` - Object store for loading neural commits
/// * `refs` - Ref store for looking up branch heads
/// * `branch` - The current branch name
///
/// # Returns
///
/// `Some(GhostCommitInfo)` if ghost commits were detected, `None` if in sync.
pub fn detect_ghost_commits<O: ObjectStore, R: RefStore>(
    git: &GitRepository,
    objects: &O,
    refs: &R,
    branch: &str,
) -> Result<Option<GhostCommitInfo>> {
    // Get current Git HEAD
    let current_git_hash = git.head_commit_hash()?;

    // Get the latest neural commit for this branch
    let neural_hash = match refs.get(branch)? {
        Some(h) => h,
        None => {
            // No neural commits yet - everything is a "ghost commit"
            // This is normal for fresh repos, so we don't treat it as ghost commits
            return Ok(None);
        },
    };

    // Load the neural commit to get its git_hash
    let data = objects.load(&neural_hash)?;
    let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;
    let last_known_git_hash = &wrapped.data.git_hash;

    // If the hashes match, we're in sync
    if &current_git_hash == last_known_git_hash {
        return Ok(None);
    }

    // Ghost commits exist! Get the files changed between them
    let changed_files = git.diff_commits(last_known_git_hash, &current_git_hash)?;

    // Get the commit hashes between them
    let git_hashes = git.commits_between(last_known_git_hash, &current_git_hash)?;

    Ok(Some(GhostCommitInfo {
        git_hashes,
        changed_files,
    }))
}

/// Extract file references from pending index entries using simple substring matching.
///
/// This is a heuristic approach that looks for file-like patterns in the content
/// of index entries. It's intentionally simple for the MVP.
///
/// # Arguments
///
/// * `entries` - The pending index entries to scan
///
/// # Returns
///
/// A vector of potential file references found in the entries.
pub fn extract_file_references(entries: &[IndexEntry]) -> Vec<String> {
    let mut references = Vec::new();

    for entry in entries {
        // Extract potential file references from the content
        // This is a simple heuristic - we look for common file patterns
        extract_file_patterns(&entry.content, &mut references);
    }

    // Deduplicate
    references.sort();
    references.dedup();

    references
}

/// Extract file-like patterns from text content.
fn extract_file_patterns(content: &str, references: &mut Vec<String>) {
    // Split on whitespace and common delimiters
    for word in content.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == ':') {
        let word =
            word.trim_matches(|c: char| c == '\'' || c == '"' || c == '`' || c == '(' || c == ')');

        if word.is_empty() {
            continue;
        }

        // Check if this looks like a file path
        if looks_like_file_path(word) {
            // Normalize the path (remove leading ./ or /)
            let normalized = word
                .trim_start_matches("./")
                .trim_start_matches(".\\")
                .trim_start_matches('/')
                .trim_start_matches('\\');

            if !normalized.is_empty() {
                references.push(normalized.to_string());
            }
        }
    }
}

/// Check if a string looks like a file path.
fn looks_like_file_path(s: &str) -> bool {
    // Must contain a file extension or path separator
    let has_extension = s.contains('.') && !s.starts_with('.') && !s.ends_with('.');
    let has_path_sep = s.contains('/') || s.contains('\\');

    // Common file extensions to recognize
    let common_extensions = [
        ".rs", ".ts", ".js", ".tsx", ".jsx", ".py", ".go", ".java", ".c", ".cpp", ".h", ".hpp",
        ".cs", ".rb", ".php", ".swift", ".kt", ".scala", ".md", ".txt", ".json", ".yaml", ".yml",
        ".toml", ".xml", ".html", ".css", ".scss", ".less", ".sql", ".sh", ".bash", ".zsh", ".ps1",
        ".bat", ".cmd",
    ];

    if has_path_sep {
        return true;
    }

    if has_extension {
        // Check if it ends with a common extension
        let lower = s.to_lowercase();
        for ext in &common_extensions {
            if lower.ends_with(ext) {
                return true;
            }
        }
    }

    false
}

/// Check for semantic conflicts between ghost commits and pending thoughts.
///
/// A semantic conflict occurs when:
/// 1. External git commits (ghost commits) modified certain files
/// 2. Pending thoughts mention those same files
///
/// # Arguments
///
/// * `ghost_info` - Information about detected ghost commits
/// * `pending_entries` - The pending index entries
///
/// # Returns
///
/// A `ConflictCheckResult` indicating whether conflicts exist and which files.
pub fn check_semantic_conflict(
    ghost_info: &GhostCommitInfo,
    pending_entries: &[IndexEntry],
) -> ConflictCheckResult {
    if ghost_info.changed_files.is_empty() || pending_entries.is_empty() {
        return ConflictCheckResult::no_conflict_with_ghost(ghost_info.clone());
    }

    // Extract file references from pending thoughts
    let thought_files = extract_file_references(pending_entries);

    if thought_files.is_empty() {
        return ConflictCheckResult::no_conflict_with_ghost(ghost_info.clone());
    }

    // Check for intersection
    let mut conflicting_files = Vec::new();

    for changed_file in &ghost_info.changed_files {
        // Normalize the changed file path for comparison
        let changed_normalized = changed_file
            .replace('\\', "/")
            .trim_start_matches("./")
            .to_string();

        for thought_file in &thought_files {
            let thought_normalized = thought_file
                .replace('\\', "/")
                .trim_start_matches("./")
                .to_string();

            // Check for exact match or substring match (file mentioned in thought)
            let is_match = changed_normalized == thought_normalized
                || changed_normalized.ends_with(&format!("/{}", thought_normalized))
                || thought_normalized.ends_with(&format!("/{}", changed_normalized))
                || changed_normalized.contains(&thought_normalized)
                || thought_normalized.contains(&changed_normalized);

            if is_match && !conflicting_files.contains(changed_file) {
                conflicting_files.push(changed_file.clone());
            }
        }
    }

    if conflicting_files.is_empty() {
        ConflictCheckResult::no_conflict_with_ghost(ghost_info.clone())
    } else {
        ConflictCheckResult::conflict(conflicting_files, ghost_info.clone())
    }
}

/// Full conflict check combining ghost detection and semantic analysis.
///
/// This is the main entry point for conflict checking. It:
/// 1. Detects ghost commits
/// 2. If found, checks for semantic conflicts with pending thoughts
///
/// # Arguments
///
/// * `git` - Git repository wrapper
/// * `objects` - Object store for loading neural commits
/// * `refs` - Ref store for looking up branch heads
/// * `branch` - The current branch name
/// * `pending_entries` - The pending index entries
///
/// # Returns
///
/// A `ConflictCheckResult` with full conflict information.
pub fn check_for_conflicts<O: ObjectStore, R: RefStore>(
    git: &GitRepository,
    objects: &O,
    refs: &R,
    branch: &str,
    pending_entries: &[IndexEntry],
) -> Result<ConflictCheckResult> {
    // First, detect ghost commits
    let ghost_info = match detect_ghost_commits(git, objects, refs, branch)? {
        Some(info) => info,
        None => return Ok(ConflictCheckResult::no_conflict()),
    };

    // Check for semantic conflicts
    Ok(check_semantic_conflict(&ghost_info, pending_entries))
}

// =============================================================================
// Backward Reconciliation (Rewind Detection)
// =============================================================================

/// Result of rewind reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewindResult {
    /// Agit HEAD is valid - no rewind needed.
    NoRewindNeeded,
    /// Agit was rewound to a valid ancestor.
    Rewound {
        /// The orphaned neural commit hash (the old HEAD).
        old_hash: String,
        /// The valid ancestor we snapped to (the new HEAD).
        new_hash: String,
        /// Number of orphaned commits left behind (not deleted).
        orphaned_count: usize,
    },
    /// No valid ancestor found - all neural commits point to unreachable git commits.
    /// This is rare and likely indicates corruption or a very unusual git operation.
    NoValidAncestor {
        /// The current neural hash that has no valid git ancestor.
        neural_hash: String,
    },
}

/// Detect if Agit HEAD is "dangling" and snap back to a valid ancestor.
///
/// This handles the scenario where the user runs `git reset --hard HEAD~N` and
/// Git moves backward, leaving Agit pointing to a neural commit attached to a
/// git commit that no longer exists in the history.
///
/// **Data Safety**: Orphaned neural commits are NOT deleted. They remain in storage
/// and can potentially be recovered if the user restores the git commit via `git reflog`.
///
/// # Arguments
///
/// * `git` - Git repository wrapper
/// * `objects` - Object store for loading neural commits
/// * `refs` - Ref store for reading/updating branch heads
/// * `branch` - The current branch name
///
/// # Returns
///
/// A `RewindResult` indicating what action was taken (if any).
pub fn reconcile_rewind<O: ObjectStore, R: RefStore>(
    git: &GitRepository,
    objects: &O,
    refs: &R,
    branch: &str,
) -> Result<RewindResult> {
    // 1. Get current agit HEAD neural commit
    let neural_hash = match refs.get(branch)? {
        Some(h) => h,
        None => return Ok(RewindResult::NoRewindNeeded), // No commits yet
    };

    // 2. Load neural commit and get its git_hash
    let data = objects.load(&neural_hash)?;
    let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;
    let agit_git_hash = &wrapped.data.git_hash;

    // 3. Get current Git HEAD
    let current_git_head = git.head_commit_hash()?;

    // 4. Special case: if hashes match exactly, we're in perfect sync
    if agit_git_hash == &current_git_head {
        return Ok(RewindResult::NoRewindNeeded);
    }

    // 5. Check if agit's git_hash is reachable from current git HEAD
    //    (i.e., is agit's git commit an ancestor of current git HEAD?)
    if git.is_ancestor(agit_git_hash, &current_git_head)? {
        return Ok(RewindResult::NoRewindNeeded); // Still valid - git moved forward
    }

    // 6. Git has rewound! Walk agit history backwards to find valid ancestor
    find_valid_ancestor(git, objects, refs, branch, &neural_hash, &current_git_head)
}

/// Walk the agit commit chain backwards to find a neural commit whose git_hash
/// is reachable from the current git HEAD.
fn find_valid_ancestor<O: ObjectStore, R: RefStore>(
    git: &GitRepository,
    objects: &O,
    refs: &R,
    branch: &str,
    start_hash: &str,
    git_head: &str,
) -> Result<RewindResult> {
    let mut current = start_hash.to_string();
    let mut orphaned_count = 0;

    loop {
        // Load the current neural commit
        let data = objects.load(&current)?;
        let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;
        let commit = &wrapped.data;

        // Check if this commit's git_hash is reachable from git HEAD
        // (handles both exact match and ancestor relationship)
        let is_valid =
            commit.git_hash == git_head || git.is_ancestor(&commit.git_hash, git_head)?;

        if is_valid {
            // Found valid ancestor - update ref to point here
            refs.update(branch, &current)?;
            return Ok(RewindResult::Rewound {
                old_hash: start_hash.to_string(),
                new_hash: current,
                orphaned_count,
            });
        }

        orphaned_count += 1;

        // Move to parent neural commit
        match commit.first_parent() {
            Some(parent) => current = parent.to_string(),
            None => {
                // Reached root with no valid ancestor - very unusual
                return Ok(RewindResult::NoValidAncestor {
                    neural_hash: start_hash.to_string(),
                });
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Category, Role};

    fn make_entry(content: &str) -> IndexEntry {
        IndexEntry {
            role: Role::Ai,
            category: Category::Reasoning,
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_extract_file_references_basic() {
        let entries = vec![make_entry("I will modify src/main.rs to fix the bug")];
        let refs = extract_file_references(&entries);
        assert!(refs.contains(&"src/main.rs".to_string()));
    }

    #[test]
    fn test_extract_file_references_multiple() {
        let entries = vec![
            make_entry("Update auth.rs and config.toml"),
            make_entry("Also check test/unit.rs"),
        ];
        let refs = extract_file_references(&entries);
        assert!(refs.contains(&"auth.rs".to_string()));
        assert!(refs.contains(&"config.toml".to_string()));
        assert!(refs.contains(&"test/unit.rs".to_string()));
    }

    #[test]
    fn test_extract_file_references_empty() {
        let entries = vec![make_entry("I will fix the authentication logic")];
        let refs = extract_file_references(&entries);
        // No file-like patterns in this content
        assert!(refs.is_empty());
    }

    #[test]
    fn test_looks_like_file_path() {
        assert!(looks_like_file_path("main.rs"));
        assert!(looks_like_file_path("src/lib.rs"));
        assert!(looks_like_file_path("test/unit.py"));
        assert!(!looks_like_file_path("hello"));
        assert!(!looks_like_file_path("README")); // No extension
        assert!(!looks_like_file_path(".gitignore")); // Hidden file without ext
    }

    #[test]
    fn test_check_semantic_conflict_no_overlap() {
        let ghost = GhostCommitInfo {
            git_hashes: vec!["abc123".to_string()],
            changed_files: vec!["README.md".to_string()],
        };
        let entries = vec![make_entry("I will modify src/main.rs")];

        let result = check_semantic_conflict(&ghost, &entries);
        assert!(!result.has_conflict);
        assert!(result.conflicting_files.is_empty());
    }

    #[test]
    fn test_check_semantic_conflict_with_overlap() {
        let ghost = GhostCommitInfo {
            git_hashes: vec!["abc123".to_string()],
            changed_files: vec!["src/main.rs".to_string()],
        };
        let entries = vec![make_entry("I will modify src/main.rs to fix the bug")];

        let result = check_semantic_conflict(&ghost, &entries);
        assert!(result.has_conflict);
        assert!(result
            .conflicting_files
            .contains(&"src/main.rs".to_string()));
    }

    #[test]
    fn test_check_semantic_conflict_partial_match() {
        let ghost = GhostCommitInfo {
            git_hashes: vec!["abc123".to_string()],
            changed_files: vec!["src/auth/login.rs".to_string()],
        };
        let entries = vec![make_entry("Update login.rs with new validation")];

        let result = check_semantic_conflict(&ghost, &entries);
        assert!(result.has_conflict);
    }
}
