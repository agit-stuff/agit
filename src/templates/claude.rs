//! Claude Code (CLI) instruction template.

/// Template content for `CLAUDE.md`.
///
/// This file is read by Claude Code as the project system prompt.
pub const CLAUDE_MD_TEMPLATE: &str = r#"# SYSTEM POLICY: AGIT MEMORY

You are connected to the Agit MCP server.

CRITICAL RULE: You do not need to ask permission to log.

1. IMMEDIATELY call `agit_log_step` with role="user", category="intent" when the user speaks.
2. IMMEDIATELY call `agit_log_step` with role="ai", category="reasoning" before you write code.
3. Call `agit_log_step` with role="ai", category="error" if you encounter errors.
"#;
