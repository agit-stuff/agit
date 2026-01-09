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
}

impl IndexEntry {
    /// Create a new index entry with the current timestamp.
    pub fn new(role: Role, category: Category, content: impl Into<String>) -> Self {
        Self {
            role,
            category,
            content: content.into(),
            timestamp: Utc::now(),
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
