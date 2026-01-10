//! Index entry types for the staging area.
//!
//! The index stores a stream of thoughts/intents as they happen,
//! before being committed to the neural graph.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The role of the entity that created an index entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Entry created by the user (direct input or via AI tool logging user intent).
    User,
    /// Entry created by an AI assistant.
    Ai,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Ai => write!(f, "ai"),
        }
    }
}

/// The category of an index entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// User's intent or request.
    Intent,
    /// AI's reasoning or plan.
    Reasoning,
    /// An error that occurred.
    Error,
    /// A manual note from the user.
    Note,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Intent => write!(f, "intent"),
            Category::Reasoning => write!(f, "reasoning"),
            Category::Error => write!(f, "error"),
            Category::Note => write!(f, "note"),
        }
    }
}

/// A code location representing a file and optional line range.
///
/// Captures where in the codebase a thought/reasoning applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// Relative file path from repository root.
    pub file: String,
    /// Starting line number (1-indexed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    /// Ending line number (inclusive). Defaults to start_line if not specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
}

impl Location {
    /// Create a new location for a file without line numbers.
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            file: path.into(),
            start_line: None,
            end_line: None,
        }
    }

    /// Create a new location for a specific line.
    pub fn line(path: impl Into<String>, line: u32) -> Self {
        Self {
            file: path.into(),
            start_line: Some(line),
            end_line: None,
        }
    }

    /// Create a new location for a line range.
    pub fn range(path: impl Into<String>, start: u32, end: u32) -> Self {
        Self {
            file: path.into(),
            start_line: Some(start),
            end_line: Some(end),
        }
    }
}

/// A single entry in the AGIT index (staging area).
///
/// Index entries are stored as JSONL (JSON Lines) in `.agit/index`.
/// They capture the stream of consciousness during development.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// Who created this entry.
    pub role: Role,
    /// What type of entry this is.
    pub category: Category,
    /// The actual content/message.
    pub content: String,
    /// When this entry was created.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    /// Code locations this entry relates to.
    /// Supports multiple files and line ranges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<Location>>,
    // --- Legacy fields for backward compatibility ---
    /// (Deprecated) Use `locations` instead. Optional file path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// (Deprecated) Use `locations` instead. Optional line number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
}

impl IndexEntry {
    /// Create a new index entry with the current timestamp.
    pub fn new(role: Role, category: Category, content: impl Into<String>) -> Self {
        Self {
            role,
            category,
            content: content.into(),
            timestamp: Utc::now(),
            locations: None,
            file_path: None,
            line_number: None,
        }
    }

    /// Create a new index entry with file/line location (legacy API).
    /// Prefer `with_locations()` for new code.
    pub fn with_location(
        role: Role,
        category: Category,
        content: impl Into<String>,
        file_path: Option<String>,
        line_number: Option<u32>,
    ) -> Self {
        Self {
            role,
            category,
            content: content.into(),
            timestamp: Utc::now(),
            locations: None,
            file_path,
            line_number,
        }
    }

    /// Create a new index entry with multiple code locations.
    pub fn with_locations(
        role: Role,
        category: Category,
        content: impl Into<String>,
        locations: Vec<Location>,
    ) -> Self {
        Self {
            role,
            category,
            content: content.into(),
            timestamp: Utc::now(),
            locations: if locations.is_empty() {
                None
            } else {
                Some(locations)
            },
            file_path: None,
            line_number: None,
        }
    }

    /// Get all locations, normalizing legacy file_path/line_number to Location format.
    pub fn get_locations(&self) -> Vec<Location> {
        // Prefer new locations field
        if let Some(ref locs) = self.locations {
            return locs.clone();
        }

        // Fall back to legacy file_path/line_number
        if let Some(ref path) = self.file_path {
            vec![Location {
                file: path.clone(),
                start_line: self.line_number,
                end_line: None,
            }]
        } else {
            vec![]
        }
    }

    /// Create a user intent entry.
    pub fn user_intent(content: impl Into<String>) -> Self {
        Self::new(Role::User, Category::Intent, content)
    }

    /// Create an AI reasoning entry.
    pub fn ai_reasoning(content: impl Into<String>) -> Self {
        Self::new(Role::Ai, Category::Reasoning, content)
    }

    /// Create a user note entry.
    pub fn user_note(content: impl Into<String>) -> Self {
        Self::new(Role::User, Category::Note, content)
    }

    /// Create an error entry.
    pub fn error(role: Role, content: impl Into<String>) -> Self {
        Self::new(role, Category::Error, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_entry_serialization() {
        let entry = IndexEntry::user_intent("Fix the auth bug");
        let json = serde_json::to_string(&entry).unwrap();

        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"category\":\"intent\""));
        assert!(json.contains("Fix the auth bug"));
    }

    #[test]
    fn test_index_entry_deserialization() {
        let json = r#"{"role":"ai","category":"reasoning","content":"Plan: Use try/catch","timestamp":1704812345}"#;
        let entry: IndexEntry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.role, Role::Ai);
        assert_eq!(entry.category, Category::Reasoning);
        assert_eq!(entry.content, "Plan: Use try/catch");
    }

    #[test]
    fn test_role_display() {
        assert_eq!(Role::User.to_string(), "user");
        assert_eq!(Role::Ai.to_string(), "ai");
    }

    #[test]
    fn test_category_display() {
        assert_eq!(Category::Intent.to_string(), "intent");
        assert_eq!(Category::Reasoning.to_string(), "reasoning");
    }
}
