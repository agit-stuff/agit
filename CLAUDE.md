# SYSTEM POLICY: AGIT MEMORY

<system_protocol version="1.3.0">

  <critical_rule id="BATCH_LOGGING">
    <status>MANDATORY</status>
    <trigger>TASK_COMPLETION</trigger>
    <instruction>
      You are operating under a STRICT SILENCE PROTOCOL.
      1. You **MUST NOT** call `agit_log_step` during execution (thinking, editing).
      2. You **MUST** buffer all user intents and reasoning internally.
      3. **IMMEDIATELY** after completing code changes (and BEFORE final text), you **MUST** call `agit_log_step` exactly ONCE with the `batch` parameter.
      4. **INCLUDE LOCATIONS**: When your reasoning relates to specific code, include a `locations` array with file paths and line ranges.
    </instruction>
    <schema>
      Each batch entry supports these fields:
      - role: "user" | "ai" (REQUIRED)
      - category: "intent" | "reasoning" | "error" (REQUIRED)
      - content: string (REQUIRED)
      - locations: array of Location objects (OPTIONAL - use when referencing specific code)

      Location object:
      - file: string (REQUIRED - relative path from repo root, e.g. "src/main.rs")
      - start_line: number (OPTIONAL - starting line number, 1-indexed)
      - end_line: number (OPTIONAL - ending line number, inclusive)
    </schema>
    <strict_binding>
      WARNING - STRICT BINDING ENFORCED:
      When `agit commit` runs, memories are validated against git staged files:
      - Memories WITH locations: AUTO-PRUNED if their files are NOT in git staging area
      - Memories WITHOUT locations: Always kept (treated as general context)

      CONSEQUENCE: If you log reasoning about "src/auth.rs" but the user reverts that file
      before committing, your memory will be DELETED. To preserve your reasoning:
      1. Include locations for file-specific changes
      2. Omit locations for general decisions/architecture notes
    </strict_binding>
    <example_payload>
      {
        "batch": [
          { "role": "user", "category": "intent", "content": "Refactor authentication module" },
          {
            "role": "ai",
            "category": "reasoning",
            "content": "Restructured token validation and added session handling",
            "locations": [
              { "file": "src/auth.rs", "start_line": 42, "end_line": 65 },
              { "file": "src/auth.rs", "start_line": 120, "end_line": 135 }
            ]
          }
        ]
      }
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
