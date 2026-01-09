//! Claude Code (CLI) instruction template.

/// Template content for `CLAUDE.md`.
///
/// This file is read by Claude Code as the project system prompt.
pub const CLAUDE_MD_TEMPLATE: &str = r#"# CLAUDE.md - Project Context & Rules

## Memory Integration (AGIT)

You are connected to the `agit` MCP server, which acts as the long-term memory for this project.

### CRITICAL RULES

1. **Log User Intent:** When the user gives you a task, IMMEDIATELY call the tool `agit_log_step` with:
   - `role`: "user"
   - `category`: "intent"
   - `content`: The user's request (summarized if very long)

2. **Log Your Plan:** BEFORE you write any code or edit files, call `agit_log_step` with:
   - `role`: "ai"
   - `category`: "reasoning"
   - `content`: Your step-by-step plan for accomplishing the task

3. **Log Errors:** If you encounter an error or obstacle, call `agit_log_step` with:
   - `role`: "ai"
   - `category`: "error"
   - `content`: Description of the error and your approach to fix it

4. **Read History:** If you are unsure of the current task or context:
   - Call `agit_read_roadmap` to understand project goals
   - Call `agit_get_recent_summaries` to see what was done recently

### Why This Matters

Every thought you log becomes part of the project's permanent history. When someone runs `agit log`, they will see not just WHAT changed, but WHY it changed and HOW the decision was made.

This creates a "Neural Graph" alongside the Git graph - code is the artifact, context is the source.
"#;
