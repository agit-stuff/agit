//! Cursor instruction template.

/// Template content for `.cursorrules`.
///
/// This file is read by Cursor as project-specific agent rules.
pub const CURSORRULES_TEMPLATE: &str = r#"# SYSTEM POLICY: AGIT MEMORY

You are connected to the Agit MCP server.

CRITICAL RULE: You do not need to ask permission to log.

1. IMMEDIATELY call `agit_log_step` with role="user", category="intent" when the user speaks.
2. IMMEDIATELY call `agit_log_step` with role="ai", category="reasoning" before you write code.
3. Call `agit_log_step` with role="ai", category="error" if you encounter errors.

## RETRIEVAL PROTOCOL

Before making assumptions about past decisions, search the project history:

- If the user asks "Why did we...", "What was the reason...", or similar questions about past decisions:
  ALWAYS call `agit_get_relevant_context` with a relevant query BEFORE answering.

- If you are unsure why a specific pattern or approach was used in existing code:
  Call `agit_get_relevant_context` to find the original reasoning.

- When modifying existing code that has non-obvious patterns:
  Search for context first to understand the intent behind the current implementation.

Example: `agit_get_relevant_context(query="authentication JWT")` to find why JWT was chosen.

## AUTO-CONTEXT INJECTION

When you start working on or reading a file, proactively gather context:

- BEFORE modifying any file, call `agit_get_file_history` with the filepath to understand
  past changes and reasoning behind the current implementation.

- When you open a file and see patterns you don't understand, call `agit_get_file_history`
  to discover WHY the code was written that way.

- This is especially important for:
  * Files with non-obvious patterns or workarounds
  * Configuration files with specific settings
  * Core modules that other code depends on

Example: `agit_get_file_history(filepath="src/auth/jwt.rs")` before modifying authentication code.
"#;
