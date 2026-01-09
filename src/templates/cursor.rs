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
"#;
