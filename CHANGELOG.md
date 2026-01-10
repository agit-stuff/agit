# Changelog

All notable changes to AGIT will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.3.0] - 2026-01-10

### Added
- **Git hooks for automatic agit sync** - Automatically synchronize agit state with git operations
  - Post-commit hook to sync neural context after commits
  - Post-merge hook to sync after merges/pulls
  - Pre-push hook validation
- **`agit reset` command** - Clear pending thoughts from the staging area
  - Useful for starting fresh without committing incomplete context
- **`--update` flag for `agit init`** - Upgrade AI instruction protocols in-place
  - Updates CLAUDE.md, .cursorrules, .windsurfrules to latest templates
  - Preserves custom sections while upgrading system policies
- **`link_to_existing_commit` pipeline method** - Link neural context to existing git commits
  - Enables retroactive context attachment
  - Useful for adding reasoning to commits made outside agit

### Fixed
- **Stricter file existence validation** - Require file existence in path validation to prevent cross-repo contamination
  - Blocks attempts to reference files that don't exist in the repository

## [1.2.0] - 2025-01-10

### Added
- **Conscious Commit protocol** - Ensures Journal Entries (empty commits) are only created when explicitly intended
  - New `--journal` flag (alias: `--allow-empty`) for `agit commit`
  - TTY-aware Safety Interceptor: prompts for confirmation in interactive terminals
  - Non-interactive environments require `--journal` flag (prevents CI hangs)
  - Supports truly empty journal entries for decision checkpoints
- **Strict scope enforcement** - Prevents cross-repository context contamination
  - Path validation in `agit_log_step` blocks path traversal attacks
  - Validates all `file_path` values are within repository root
  - New `PathOutsideRepository` error with clear security messaging
- **File/line location tracking** - Neural context entries can now reference specific code locations
  - New `--file` and `--line` flags for `agit record`
  - MCP tool `agit_log_step` supports `file_path` and `line_number` fields
  - VS Code extension displays context at specific line numbers via CodeLens
- **Per-branch index stashing** - Automatically stash/restore pending thoughts when switching branches
  - Pending thoughts preserved per-branch in `.agit/stash/`
  - Seamless context switching without losing work-in-progress reasoning
- **Git amend detection** - Detects `git commit --amend` and migrates neural memory
  - Rewrites parent references to maintain graph integrity
  - Preserves reasoning history even when commit hashes change
- **Merge/rebase conflict guard** - Blocks mutating Agit commands during git conflicts
  - Prevents neural graph corruption during unfinished merge/rebase operations
  - Clear error messages guide users to resolve conflicts first

### Changed
- Journal Entry commits now use `[Agit] Journal:` prefix instead of `[Agit] Context Update:`
- V2 pipeline now creates empty git commits for memory-only changes (visible in `git log`)

## [1.1.1] - 2025-01-10

### Added
- **Semantic conflict detection (Safety Valve)** - Prevents accidental overwrites when external commits touch files mentioned in pending thoughts
  - Detects "ghost commits" (git commits made outside Agit)
  - Blocks commit if ghost commits modified files referenced in pending thoughts
  - New `--force` flag to override the safety check
  - Warnings displayed in `agit status` when conflicts exist
- **Bidirectional reconciliation** - Handles both forward and backward git history changes
  - Forward: detects ghost commits (existing)
  - Backward: detects git rewinds (`git reset --hard`) and snaps Agit HEAD to valid ancestor
  - Orphaned neural commits preserved for potential recovery via `git reflog`
- **Auto-rebuild search index after pull** - Eliminates need for manual `agit search rebuild`
  - Incremental indexing for pulled commits
  - Falls back to full rebuild if index is unhealthy
  - Non-fatal: warns on failure but doesn't block pull

### Changed
- **Upgraded system policy templates** - Stricter enforcement using XML + RFC 2119 language
  - BATCH_LOGGING: Mandatory silence protocol for `agit_log_step`
  - RETRIEVAL_VERIFICATION: Required context lookup for history questions
  - CONTEXT_INJECTION: Blocking rule for file modifications without history check

## [1.0.0] - 2025-01-10

### Added
- **Full-text search with Tantivy** - Fuzzy-search past reasoning logs instead of reading entire history
  - New `src/search/` module with indexer and retriever
  - Search index stored in `.agit/search_index/` (local-only)
  - Automatic indexing during `agit commit`
- **New CLI commands:**
  - `agit search rebuild` - Rebuild search index from existing neural commits
  - `agit search query <query>` - Query the search index (for testing)
- **New MCP tools:**
  - `agit_get_relevant_context` - Search past reasoning logs by keywords
  - `agit_get_file_history` - Get history of changes to a specific file (auto-context injection)
- **Enhanced instruction templates:**
  - Added RETRIEVAL PROTOCOL section - instructs agents when to search history
  - Added AUTO-CONTEXT INJECTION section - instructs agents to check file history before modifications

### Dependencies
- Added `tantivy = "0.22"` for full-text search

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

[Unreleased]: https://github.com/agit-stuff/agit/compare/v1.3.0...HEAD
[1.3.0]: https://github.com/agit-stuff/agit/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/agit-stuff/agit/compare/v1.1.1...v1.2.0
[1.1.1]: https://github.com/agit-stuff/agit/compare/v1.0.0...v1.1.1
[1.0.0]: https://github.com/agit-stuff/agit/compare/v0.3.0...v1.0.0
[0.3.0]: https://github.com/agit-stuff/agit/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/agit-stuff/agit/releases/tag/v0.2.0
