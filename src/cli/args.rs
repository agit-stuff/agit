//! CLI argument definitions using clap.

use clap::{Parser, Subcommand};

/// AGIT: AI-Native Git Wrapper
///
/// Captures reasoning context alongside code changes.
/// "Code is the Artifact. Context is the Source."
#[derive(Parser, Debug)]
#[command(name = "agit")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// The command to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available AGIT commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize AGIT in the current directory
    Init(InitArgs),

    /// Record a thought or note to the staging area
    Record(RecordArgs),

    /// Show the current status
    Status(StatusArgs),

    /// Show the neural commit history
    Log(LogArgs),

    /// Show details of a specific commit
    Show(ShowArgs),

    /// Create a commit with linked neural context
    Commit(CommitArgs),

    /// Stage files and freeze context for commit
    Add(AddArgs),

    /// Push agit refs to a remote repository
    Push(PushArgs),

    /// Pull agit refs from a remote repository
    Pull(PullArgs),

    /// Migrate storage from V1 (file-based) to V2 (Git-native)
    Migrate(MigrateArgs),

    /// Start the MCP server for AI editors
    Serve(ServeArgs),

    /// Search-related commands (rebuild index, query)
    Search(SearchArgs),

    /// Reset (clear) pending thoughts and staged context
    Reset(ResetArgs),

    /// Sync agit state with current git state
    Sync(SyncArgs),

    /// Manage git hooks for automatic sync
    Hooks(HooksArgs),
}

/// Arguments for the `init` command
#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Force initialization even if already initialized
    #[arg(short, long)]
    pub force: bool,

    /// Skip generating AI instruction files (CLAUDE.md, .cursorrules, etc.)
    #[arg(long)]
    pub no_templates: bool,

    /// Skip updating .gitignore
    #[arg(long)]
    pub no_gitignore: bool,

    /// Update AI instruction files to the latest version without destroying custom settings
    #[arg(short, long)]
    pub update: bool,

    /// Skip git hook installation
    #[arg(long)]
    pub no_hooks: bool,
}

/// Arguments for the `record` command
#[derive(Parser, Debug)]
pub struct RecordArgs {
    /// The thought or note to record
    pub message: String,

    /// Record as AI reasoning instead of user note
    #[arg(short = 'a', long)]
    pub ai: bool,

    /// Record as intent instead of note
    #[arg(short, long)]
    pub intent: bool,

    /// File path this record relates to (e.g., "src/auth.rs")
    #[arg(short, long)]
    pub file: Option<String>,

    /// Line number in the file
    #[arg(short, long)]
    pub line: Option<u32>,
}

/// Arguments for the `status` command
#[derive(Parser, Debug)]
pub struct StatusArgs {
    /// Show verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Arguments for the `log` command
#[derive(Parser, Debug)]
pub struct LogArgs {
    /// Number of commits to show
    #[arg(short = 'n', long, default_value = "10")]
    pub count: usize,

    /// Show only the summary, not the full trace
    #[arg(short, long)]
    pub oneline: bool,
}

/// Arguments for the `show` command
#[derive(Parser, Debug)]
pub struct ShowArgs {
    /// The git commit hash (or prefix) to show context for
    pub hash: Option<String>,

    /// Show the trace content
    #[arg(short, long)]
    pub trace: bool,

    /// Show the roadmap
    #[arg(short, long)]
    pub roadmap: bool,
}

/// Arguments for the `commit` command
#[derive(Parser, Debug)]
pub struct CommitArgs {
    /// The commit message
    #[arg(short, long)]
    pub message: Option<String>,

    /// Open editor for commit message
    #[arg(short, long)]
    pub edit: bool,

    /// Edit the summary before committing
    #[arg(long)]
    pub edit_summary: bool,

    /// Amend the previous commit
    #[arg(long)]
    pub amend: bool,

    /// Skip interactive prompts (for memory-only commits)
    #[arg(short, long)]
    pub yes: bool,

    /// Force commit even if semantic conflicts are detected
    ///
    /// By default, Agit blocks commits when external git changes conflict with
    /// pending thoughts. Use this flag to proceed anyway.
    #[arg(short, long)]
    pub force: bool,

    /// Allow recording a memory without code changes (Journal Entry)
    ///
    /// Creates an empty git commit to preserve the decision history even when
    /// no code changes are present. Required in non-interactive environments.
    #[arg(long, visible_alias = "allow-empty")]
    pub journal: bool,
}

/// Arguments for the `serve` command
#[derive(Parser, Debug)]
pub struct ServeArgs {
    /// Port to listen on (for HTTP mode, not implemented yet)
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Run in verbose mode
    #[arg(short, long)]
    pub verbose: bool,
}

/// Arguments for the `add` command
#[derive(Parser, Debug)]
pub struct AddArgs {
    /// Files or patterns to add (e.g., ".", "src/")
    #[arg(default_value = ".")]
    pub pathspec: Vec<String>,
}

/// Arguments for the `push` command
#[derive(Parser, Debug)]
pub struct PushArgs {
    /// Remote name (default: origin)
    #[arg(default_value = "origin")]
    pub remote: String,

    /// Force push (overwrite remote refs)
    #[arg(short, long)]
    pub force: bool,
}

/// Arguments for the `pull` command
#[derive(Parser, Debug)]
pub struct PullArgs {
    /// Remote name (default: origin)
    #[arg(default_value = "origin")]
    pub remote: String,
}

/// Arguments for the `migrate` command
#[derive(Parser, Debug)]
pub struct MigrateArgs {
    /// Clean up old storage after migration
    #[arg(long)]
    pub cleanup: bool,

    /// Force migration even if already on V2
    #[arg(short, long)]
    pub force: bool,
}

/// Arguments for the `search` command
#[derive(Parser, Debug)]
pub struct SearchArgs {
    /// Search subcommand
    #[command(subcommand)]
    pub command: SearchCommands,
}

/// Search subcommands
#[derive(Subcommand, Debug)]
pub enum SearchCommands {
    /// Rebuild the search index from existing neural commits
    Rebuild,
    /// Query the search index
    Query(SearchQueryArgs),
}

/// Arguments for search query
#[derive(Parser, Debug)]
pub struct SearchQueryArgs {
    /// The search query
    pub query: String,
    /// Maximum number of results
    #[arg(short, long, default_value = "5")]
    pub limit: usize,
}

/// Arguments for the `reset` command
#[derive(Parser, Debug)]
pub struct ResetArgs {
    /// Clear only pending thoughts, preserve staged context
    #[arg(long)]
    pub soft: bool,

    /// Clear all: pending, staged, AND all branch stashes
    #[arg(long)]
    pub hard: bool,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
}

/// Arguments for the `sync` command
#[derive(Parser, Debug)]
pub struct SyncArgs {
    /// The git hook that triggered this sync (for internal use)
    #[arg(long)]
    pub hook: Option<String>,

    /// Run silently (suppress non-error output)
    #[arg(short, long)]
    pub quiet: bool,
}

/// Arguments for the `hooks` command
#[derive(Parser, Debug)]
pub struct HooksArgs {
    /// Hooks subcommand
    #[command(subcommand)]
    pub command: HooksCommands,
}

/// Hooks subcommands
#[derive(Subcommand, Debug)]
pub enum HooksCommands {
    /// Install agit git hooks for automatic sync
    Install,
    /// Remove agit git hooks
    Uninstall,
    /// Show current hook installation status
    Status,
}
