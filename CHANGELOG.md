# Changelog

All notable changes to AGIT will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2025-01-10

### Added
- **V2 Git-native storage** - Agit is now invisible to standard Git workflows
  - Objects stored in Git ODB (via `repo.blob()`) instead of `.agit/objects/`
  - Branch refs stored in `refs/agit/heads/*` namespace instead of `.agit/refs/heads/`
  - Clean `git status` - no more `.agit/` changes polluting your working tree
  - Invisible in `git branch -a` and GitHub/GitLab UI
- New storage implementations:
  - `GitObjectStore` - stores objects as Git blobs
  - `GitRefStore` - manages refs in `refs/agit/heads/*` namespace
- `agit push` - Push agit refs to remote (`refs/agit/*:refs/agit/*`)
- `agit pull` - Pull agit refs from remote
- `agit migrate` - Migrate existing V1 repos to V2 storage
  - `--cleanup` flag to remove old `.agit/objects/` and `.agit/refs/` after migration
- `--yes` / `-y` flag for `agit commit` to skip interactive prompts (useful for scripts/CI)
- MCP configuration auto-setup on `agit init`:
  - `.mcp.json` for Windsurf
  - `.cursor/mcp.json` for Cursor

### Changed
- Default storage is now V2 (Git-native) for new repositories
- Memory-only commits no longer create `[Agit]` prefixed git commits in V2
- Simplified `.agit/` directory structure (only local state: HEAD, index, config.json, tmp/)

### Fixed
- Rustdoc warnings for `[Agit]` being parsed as intra-doc links

## [0.2.0] - 2024-12-15

### Added
- Initial public release
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

[Unreleased]: https://github.com/agit-stuff/agit/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/agit-stuff/agit/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/agit-stuff/agit/releases/tag/v0.2.0
