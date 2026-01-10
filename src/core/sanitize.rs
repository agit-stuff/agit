//! Sanitization logic for index entries.
//!
//! This module implements "Strict Binding" between file changes and context memory.
//! When committing, entries whose file locations don't match staged files are pruned.

use crate::domain::IndexEntry;
#[cfg(test)]
use crate::domain::Location;

/// Result of sanitizing index entries against Git staged files.
#[derive(Debug)]
pub struct SanitizeResult {
    /// Entries that match staged files (kept).
    pub kept: Vec<IndexEntry>,
    /// Entries that were pruned (orphaned).
    /// Each tuple contains the entry and the list of orphaned file paths.
    pub pruned: Vec<(IndexEntry, Vec<String>)>,
}

/// Sanitize index entries by pruning those whose locations don't match staged files.
///
/// This enforces "Strict Binding" - memories stay bound to code changes.
/// If a file is reverted/reset, its associated memories are pruned.
///
/// # Rules
///
/// - Entries with NO locations are kept (general memories)
/// - Entries with locations are kept ONLY if at least one location matches a staged file
/// - Matching is done by checking if paths are equal or one ends with the other
///
/// # Arguments
///
/// * `entries` - The index entries to sanitize
/// * `staged_files` - List of files staged for commit (from git)
///
/// # Returns
///
/// A `SanitizeResult` containing kept and pruned entries.
pub fn sanitize_entries(entries: Vec<IndexEntry>, staged_files: &[String]) -> SanitizeResult {
    let mut kept = Vec::new();
    let mut pruned = Vec::new();

    for entry in entries {
        let locations = entry.get_locations();

        if locations.is_empty() {
            // No file locations - this is a general memory, keep it
            kept.push(entry);
            continue;
        }

        // Check if any location matches a staged file
        let mut matched_any = false;
        let mut orphaned_paths = Vec::new();

        for loc in &locations {
            if location_matches_staged(&loc.file, staged_files) {
                matched_any = true;
            } else {
                orphaned_paths.push(loc.file.clone());
            }
        }

        if matched_any {
            // At least one location matches - keep the entry
            kept.push(entry);
        } else {
            // No locations match staged files - prune the entry
            pruned.push((entry, orphaned_paths));
        }
    }

    SanitizeResult { kept, pruned }
}

/// Check if a location file path matches any of the staged files.
///
/// Handles path normalization (backslash vs forward slash) and
/// partial path matching (relative vs absolute paths).
fn location_matches_staged(loc_file: &str, staged_files: &[String]) -> bool {
    let normalized = loc_file.replace('\\', "/");

    staged_files.iter().any(|staged| {
        let staged_normalized = staged.replace('\\', "/");

        // Exact match
        if staged_normalized == normalized {
            return true;
        }

        // Check if one ends with the other (handles relative path differences)
        if staged_normalized.ends_with(&format!("/{}", normalized)) {
            return true;
        }
        if normalized.ends_with(&format!("/{}", staged_normalized)) {
            return true;
        }

        // Also check without leading slash for simpler paths
        if normalized == staged_normalized.trim_start_matches('/') {
            return true;
        }
        if staged_normalized == normalized.trim_start_matches('/') {
            return true;
        }

        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Category, Role};

    #[test]
    fn test_sanitize_keeps_matching_entries() {
        let entries = vec![IndexEntry::with_locations(
            Role::Ai,
            Category::Reasoning,
            "Updated auth logic",
            vec![Location::range("src/auth.rs".to_string(), 10, 20)],
        )];
        let staged = vec!["src/auth.rs".to_string()];

        let result = sanitize_entries(entries, &staged);
        assert_eq!(result.kept.len(), 1);
        assert_eq!(result.pruned.len(), 0);
    }

    #[test]
    fn test_sanitize_prunes_orphaned_entries() {
        let entries = vec![IndexEntry::with_locations(
            Role::Ai,
            Category::Reasoning,
            "Updated auth logic",
            vec![Location::range("src/auth.rs".to_string(), 10, 20)],
        )];
        let staged = vec!["src/main.rs".to_string()]; // Different file

        let result = sanitize_entries(entries, &staged);
        assert_eq!(result.kept.len(), 0);
        assert_eq!(result.pruned.len(), 1);
        assert_eq!(result.pruned[0].1, vec!["src/auth.rs"]);
    }

    #[test]
    fn test_sanitize_keeps_general_memories() {
        let entries = vec![IndexEntry::new(
            Role::User,
            Category::Intent,
            "General decision without file location",
        )];
        let staged = vec!["src/main.rs".to_string()];

        let result = sanitize_entries(entries, &staged);
        assert_eq!(result.kept.len(), 1); // No locations = kept
        assert_eq!(result.pruned.len(), 0);
    }

    #[test]
    fn test_sanitize_partial_match_keeps_entry() {
        let entries = vec![IndexEntry::with_locations(
            Role::Ai,
            Category::Reasoning,
            "Updated multiple files",
            vec![
                Location::range("src/auth.rs".to_string(), 10, 20), // Not staged
                Location::range("src/main.rs".to_string(), 5, 10),  // Staged
            ],
        )];
        let staged = vec!["src/main.rs".to_string()];

        let result = sanitize_entries(entries, &staged);
        assert_eq!(result.kept.len(), 1); // Partial match = kept
        assert_eq!(result.pruned.len(), 0);
    }

    #[test]
    fn test_sanitize_all_orphaned_returns_all_pruned() {
        let entries = vec![
            IndexEntry::with_locations(
                Role::Ai,
                Category::Reasoning,
                "Memory for reverted file",
                vec![Location::file("src/reverted.rs".to_string())],
            ),
            IndexEntry::with_locations(
                Role::User,
                Category::Intent,
                "Intent for another reverted file",
                vec![Location::line("src/another.rs".to_string(), 5)],
            ),
        ];
        let staged = vec!["src/main.rs".to_string()]; // Neither matches

        let result = sanitize_entries(entries, &staged);
        assert_eq!(result.kept.len(), 0);
        assert_eq!(result.pruned.len(), 2);
    }

    #[test]
    fn test_sanitize_mixed_entries() {
        let entries = vec![
            // General memory (no location) - should be kept
            IndexEntry::new(Role::User, Category::Intent, "General intent"),
            // Memory for staged file - should be kept
            IndexEntry::with_locations(
                Role::Ai,
                Category::Reasoning,
                "Logic for staged file",
                vec![Location::file("src/staged.rs".to_string())],
            ),
            // Memory for reverted file - should be pruned
            IndexEntry::with_locations(
                Role::Ai,
                Category::Reasoning,
                "Logic for reverted file",
                vec![Location::file("src/reverted.rs".to_string())],
            ),
        ];
        let staged = vec!["src/staged.rs".to_string()];

        let result = sanitize_entries(entries, &staged);
        assert_eq!(result.kept.len(), 2); // General + staged file
        assert_eq!(result.pruned.len(), 1); // Reverted file
    }

    #[test]
    fn test_path_normalization_windows_backslash() {
        let entries = vec![IndexEntry::with_locations(
            Role::Ai,
            Category::Reasoning,
            "Updated with Windows path",
            vec![Location::file("src\\auth.rs".to_string())],
        )];
        let staged = vec!["src/auth.rs".to_string()]; // Unix-style

        let result = sanitize_entries(entries, &staged);
        assert_eq!(result.kept.len(), 1); // Should match despite path separator difference
    }

    #[test]
    fn test_empty_staged_files_prunes_all_with_locations() {
        let entries = vec![
            IndexEntry::new(Role::User, Category::Intent, "General memory"),
            IndexEntry::with_locations(
                Role::Ai,
                Category::Reasoning,
                "Memory with location",
                vec![Location::file("src/file.rs".to_string())],
            ),
        ];
        let staged: Vec<String> = vec![]; // No staged files

        let result = sanitize_entries(entries, &staged);
        assert_eq!(result.kept.len(), 1); // Only general memory kept
        assert_eq!(result.pruned.len(), 1); // Memory with location pruned
    }
}
