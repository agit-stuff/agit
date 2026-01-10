//! Claude Code (CLI) instruction template.

/// Template content for `CLAUDE.md`.
///
/// This file is read by Claude Code as the project system prompt.
pub const CLAUDE_MD_TEMPLATE: &str = r#"# SYSTEM POLICY: AGIT MEMORY

<system_protocol>

  <critical_rule id="BATCH_LOGGING">
    <status>MANDATORY</status>
    <trigger>TASK_COMPLETION</trigger>
    <instruction>
      You are operating under a STRICT SILENCE PROTOCOL.
      1. You **MUST NOT** call `agit_log_step` during execution (thinking, editing).
      2. You **MUST** buffer all user intents and reasoning internally.
      3. **IMMEDIATELY** after completing code changes (and BEFORE final text), you **MUST** call `agit_log_step` exactly ONCE with the `batch` parameter.
    </instruction>
    <example_payload>
      { "batch": [ { "role": "user", "...": "..." }, { "role": "ai", "...": "..." } ] }
    </example_payload>
  </critical_rule>

  <critical_rule id="RETRIEVAL_VERIFICATION">
    <status>REQUIRED</status>
    <trigger>USER_QUERY_TYPE</trigger>
    <instruction>
      You **MUST NOT** rely on internal training data for project history.
      1. If the user asks "Why", "When", "Who", or "Reason" regarding past decisions:
         - You **MUST** call `agit_get_relevant_context` BEFORE generating an answer.
         - Do not guess. If the tool returns no results, state that explicitly.
    </instruction>
  </critical_rule>

  <critical_rule id="CONTEXT_INJECTION">
    <status>BLOCKING</status>
    <trigger>FILE_MODIFICATION</trigger>
    <instruction>
      You are **FORBIDDEN** from modifying existing files without context.
      1. BEFORE writing to any non-empty file, you **MUST** call `agit_get_file_history(filepath=...)`.
      2. Review the returned history to ensure your changes do not regress previous decisions.
      3. **EXCEPTION:** Creating new files does not require history lookup.
    </instruction>
  </critical_rule>

</system_protocol>
"#;
