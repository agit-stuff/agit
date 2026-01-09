//! Summary synthesizer.
//!
//! Generates a human-readable summary from index entries.
//! This is deterministic - no LLM calls.

use crate::domain::{Category, IndexEntry, Role};

/// Summary synthesizer that creates summaries from index entries.
pub struct SynthesizeSummary;

impl SynthesizeSummary {
    /// Synthesize a summary from index entries.
    ///
    /// This extracts the last user intent and AI reasoning,
    /// formatting them into a readable summary.
    ///
    /// # Example
    ///
    /// ```
    /// use agit::domain::IndexEntry;
    /// use agit::core::SynthesizeSummary;
    ///
    /// let entries = vec![
    ///     IndexEntry::user_intent("Fix the auth bug"),
    ///     IndexEntry::ai_reasoning("Add try/catch around token validation"),
    /// ];
    ///
    /// let summary = SynthesizeSummary::synthesize(&entries);
    /// assert!(summary.contains("Fix the auth bug"));
    /// assert!(summary.contains("try/catch"));
    /// ```
    pub fn synthesize(entries: &[IndexEntry]) -> String {
        let intent = Self::find_last_intent(entries);
        let reasoning = Self::find_last_reasoning(entries);

        match (intent, reasoning) {
            (Some(i), Some(r)) => format!("Intent: {}. Plan: {}.", i, r),
            (Some(i), None) => format!("Intent: {}.", i),
            (None, Some(r)) => format!("Plan: {}.", r),
            (None, None) => "Manual update.".to_string(),
        }
    }

    /// Find the last user intent in the entries.
    fn find_last_intent(entries: &[IndexEntry]) -> Option<String> {
        entries
            .iter()
            .filter(|e| e.role == Role::User && e.category == Category::Intent)
            .next_back()
            .map(|e| Self::truncate_content(&e.content))
    }

    /// Find the last AI reasoning in the entries.
    fn find_last_reasoning(entries: &[IndexEntry]) -> Option<String> {
        entries
            .iter()
            .filter(|e| e.role == Role::Ai && e.category == Category::Reasoning)
            .next_back()
            .map(|e| Self::truncate_content(&e.content))
    }

    /// Truncate content if too long.
    fn truncate_content(content: &str) -> String {
        const MAX_LEN: usize = 200;

        let content = content.trim();

        if content.len() <= MAX_LEN {
            content.to_string()
        } else {
            format!("{}...", &content[..MAX_LEN - 3])
        }
    }

    /// Get all entries as a formatted trace string.
    pub fn format_trace(entries: &[IndexEntry]) -> String {
        entries
            .iter()
            .map(|e| {
                format!(
                    "[{}] {}/{}: {}",
                    e.timestamp.format("%H:%M:%S"),
                    e.role,
                    e.category,
                    e.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthesize_with_intent_and_reasoning() {
        let entries = vec![
            IndexEntry::user_intent("Fix the auth bug"),
            IndexEntry::ai_reasoning("Add try/catch around token validation"),
        ];

        let summary = SynthesizeSummary::synthesize(&entries);
        assert_eq!(
            summary,
            "Intent: Fix the auth bug. Plan: Add try/catch around token validation."
        );
    }

    #[test]
    fn test_synthesize_with_only_intent() {
        let entries = vec![IndexEntry::user_intent("Refactor the module")];

        let summary = SynthesizeSummary::synthesize(&entries);
        assert_eq!(summary, "Intent: Refactor the module.");
    }

    #[test]
    fn test_synthesize_with_only_reasoning() {
        let entries = vec![IndexEntry::ai_reasoning("Use a factory pattern")];

        let summary = SynthesizeSummary::synthesize(&entries);
        assert_eq!(summary, "Plan: Use a factory pattern.");
    }

    #[test]
    fn test_synthesize_empty() {
        let entries: Vec<IndexEntry> = vec![];
        let summary = SynthesizeSummary::synthesize(&entries);
        assert_eq!(summary, "Manual update.");
    }

    #[test]
    fn test_synthesize_uses_last_entries() {
        let entries = vec![
            IndexEntry::user_intent("First task"),
            IndexEntry::user_intent("Second task"),
            IndexEntry::ai_reasoning("First plan"),
            IndexEntry::ai_reasoning("Better plan"),
        ];

        let summary = SynthesizeSummary::synthesize(&entries);
        assert_eq!(summary, "Intent: Second task. Plan: Better plan.");
    }

    #[test]
    fn test_truncate_long_content() {
        let long_content = "a".repeat(300);
        let entries = vec![IndexEntry::user_intent(&long_content)];

        let summary = SynthesizeSummary::synthesize(&entries);
        assert!(summary.len() < 250);
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn test_format_trace() {
        let entries = vec![
            IndexEntry::user_intent("Do something"),
            IndexEntry::ai_reasoning("Will do it this way"),
        ];

        let trace = SynthesizeSummary::format_trace(&entries);
        assert!(trace.contains("user/intent: Do something"));
        assert!(trace.contains("ai/reasoning: Will do it this way"));
    }
}
