//! Claude Code (CLI) instruction template.

use super::{AGIT_VERSION, SYSTEM_PROTOCOL_CONTENT};

/// Generate template content for `CLAUDE.md` with current version.
///
/// This file is read by Claude Code as the project system prompt.
pub fn generate_claude_md_template() -> String {
    format!(
        r#"# SYSTEM POLICY: AGIT MEMORY

<system_protocol version="{}">
{}
</system_protocol>
"#,
        AGIT_VERSION, SYSTEM_PROTOCOL_CONTENT
    )
}

/// Static template content for `CLAUDE.md` (legacy, for compatibility).
///
/// Note: Prefer using `generate_claude_md_template()` for versioned output.
pub const CLAUDE_MD_TEMPLATE: &str = concat!(
    "# SYSTEM POLICY: AGIT MEMORY\n\n<system_protocol>\n",
    include_str!("protocol_content.txt"),
    "\n</system_protocol>\n"
);
