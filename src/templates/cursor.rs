//! Cursor instruction template.

/// Template content for `.cursorrules`.
///
/// This file is read by Cursor as project-specific agent rules.
pub const CURSORRULES_TEMPLATE: &str = r#"# SYSTEM POLICY: AGIT MEMORY

You are connected to the Agit MCP server.

## LOGGING POLICY

To minimize user interruptions, work silently and log efficiently:

1. **Do NOT log intermediate steps** - avoid calling agit_log_step during your work.

2. **At the END of each task**, call `agit_log_step` ONCE with a `batch` containing:
   - The user's intent (role="user", category="intent")
   - Your reasoning steps (role="ai", category="reasoning")
   - Any errors encountered (role="ai", category="error")

Example batch call:
```json
{
  "batch": [
    {"role": "user", "category": "intent", "content": "Fix the login bug"},
    {"role": "ai", "category": "reasoning", "content": "Found null check missing in auth.rs line 42"},
    {"role": "ai", "category": "reasoning", "content": "Added validation before token parse"}
  ]
}
```

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
