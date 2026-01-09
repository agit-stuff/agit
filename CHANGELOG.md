# Changelog

All notable changes to AGIT will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial implementation of AGIT - AI-Native Git Wrapper
- Core CLI commands:
  - `agit init` - Initialize AGIT in a git repository
  - `agit record` - Manually record thoughts to the staging area
  - `agit status` - Show current status and pending thoughts
  - `agit commit` - Create a neural commit with linked context
  - `agit log` - View commit history with summaries
  - `agit show` - Show full context for a commit
  - `agit server` - Start the MCP server
- MCP (Model Context Protocol) server:
  - `agit_log_step` - Log conversation steps from AI editors
  - `agit_read_roadmap` - Read project roadmap
  - `agit_get_context` - Get context for a git commit
- AI instruction file generation:
  - `CLAUDE.md` for Claude Code
  - `.cursorrules` for Cursor
  - `.windsurfrules` for Windsurf
- Content-addressable object storage with SHA-256 hashing
- JSONL-based index (staging area)
- Branch reference management
- Atomic file writes for data integrity
- File locking for concurrent access protection
- Branch synchronization between Git and AGIT
- Comprehensive error handling with `thiserror`
- Domain models: `IndexEntry`, `NeuralCommit`, `ObjectEnvelope`
- Storage traits: `ObjectStore`, `IndexStore`, `RefStore`
- Summary synthesizer (deterministic, no LLM calls)
- Full test suite with unit and integration tests
- Benchmark suite using Criterion

### Security
- Atomic writes prevent data corruption
- File locking prevents race conditions
- JSON-RPC input validation in MCP server
- No shell command execution with user input

## [0.1.0] - TBD

Initial release.

[Unreleased]: https://github.com/agit-stuff/agit/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/agit-stuff/agit/releases/tag/v0.1.0
