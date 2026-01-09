# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please report it responsibly.

### How to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via email to: **security@agit-stuff.dev** (or create a private security advisory on GitHub).

Include the following information:
- Type of vulnerability (e.g., path traversal, command injection, etc.)
- Full path to the affected source file(s)
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if possible)
- Impact assessment

### What to Expect

- **Acknowledgment**: We will acknowledge receipt within 48 hours.
- **Assessment**: We will assess the vulnerability and determine its severity.
- **Fix Timeline**: Critical vulnerabilities will be addressed within 7 days. Others within 30 days.
- **Disclosure**: We will coordinate with you on disclosure timing.
- **Credit**: We will credit you in the security advisory (unless you prefer to remain anonymous).

## Security Measures

AGIT implements several security measures:

### File System Safety
- Atomic writes prevent partial file corruption
- File locking prevents race conditions
- All paths are validated to prevent traversal attacks
- No shell command execution with user input

### Data Integrity
- Content-addressable storage with SHA-256 hashing
- JSON schema versioning for forward compatibility
- Checksums verify object integrity on read

### MCP Server
- JSON-RPC 2.0 protocol with strict validation
- No network exposure (stdio transport only)
- Input sanitization on all tool parameters

### Dependencies
- Regular dependency audits with `cargo-deny`
- Automated security scanning in CI
- Minimal dependency footprint

## Security Best Practices for Users

1. **Keep AGIT Updated**: Always use the latest version to get security fixes.

2. **Protect Your `.agit` Directory**: The `.agit` directory contains your neural graph history. Ensure appropriate file permissions.

3. **Review Instruction Files**: Check `CLAUDE.md`, `.cursorrules`, and `.windsurfrules` before committing them to version control.

4. **Trust Boundaries**: AGIT trusts your AI editor via MCP. Only connect editors you trust.

## Dependency Security

We use `cargo-deny` to audit dependencies. Our policy:
- No known vulnerabilities in dependencies
- License compatibility checked
- Source repository verification

Run the audit yourself:
```bash
cargo deny check
```
