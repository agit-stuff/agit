//! Cursor instruction template.

use super::{AGIT_VERSION, SYSTEM_PROTOCOL_CONTENT};

/// Generate template content for `.cursorrules` with current version.
///
/// This file is read by Cursor as project-specific agent rules.
pub fn generate_cursorrules_template() -> String {
    format!(
        r#"# SYSTEM POLICY: AGIT MEMORY

<system_protocol version="{}">
{}
</system_protocol>
"#,
        AGIT_VERSION, SYSTEM_PROTOCOL_CONTENT
    )
}

/// Static template content for `.cursorrules` (legacy, for compatibility).
///
/// Note: Prefer using `generate_cursorrules_template()` for versioned output.
pub const CURSORRULES_TEMPLATE: &str = concat!(
    "# SYSTEM POLICY: AGIT MEMORY\n\n<system_protocol>\n",
    include_str!("protocol_content.txt"),
    "\n</system_protocol>\n"
);
