//! Action definitions and handlers for TUI.
//!
//! Actions represent user intents that update the application state.

/// Actions that can be performed in the TUI.
#[derive(Debug, Clone)]
pub enum Action {
    /// Quit the TUI
    Quit,

    /// Refresh all data from storage
    Refresh,

    /// Navigate to a specific panel
    NavigateToPanel(PanelId),

    /// Navigate to next panel
    NextPanel,

    /// Navigate to previous panel
    PreviousPanel,

    /// Move selection up in current panel
    MoveUp,

    /// Move selection down in current panel
    MoveDown,

    /// Move to top of current panel
    MoveToTop,

    /// Move to bottom of current panel
    MoveToBottom,

    /// Show help overlay
    ShowHelp,

    /// Hide help overlay
    HideHelp,

    /// Commit with a message (Phase 2)
    Commit { message: String },

    /// Sync AGIT branch to Git branch (Phase 2)
    SyncBranch,

    /// Stage a file (Phase 3)
    StageFile { path: String },

    /// Unstage a file (Phase 3)
    UnstageFile { path: String },

    /// Stage all files (Phase 3)
    StageAll,

    /// Unstage all files (Phase 3)
    UnstageAll,

    /// Delete an index entry (Phase 3)
    DeleteIndexEntry { index: usize },

    /// View selected item in preview panel
    ViewInPreview,

    /// Search in current panel (Phase 4)
    Search { query: String },

    /// Filter neural commits by role (Phase 4)
    FilterByRole { role: Option<crate::domain::Role> },

    /// Filter neural commits by category (Phase 4)
    FilterByCategory { category: Option<crate::domain::Category> },
}

/// Panel identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelId {
    Status,
    Staging,
    Files,
    GitCommits,
    NeuralCommits,
    Preview,
}

impl PanelId {
    /// Get all panel IDs in order.
    pub fn all() -> Vec<PanelId> {
        vec![
            PanelId::Status,
            PanelId::Staging,
            PanelId::Files,
            PanelId::GitCommits,
            PanelId::NeuralCommits,
            PanelId::Preview,
        ]
    }

    /// Get the next panel in the cycle.
    pub fn next(self) -> Self {
        match self {
            PanelId::Status => PanelId::Staging,
            PanelId::Staging => PanelId::Files,
            PanelId::Files => PanelId::GitCommits,
            PanelId::GitCommits => PanelId::NeuralCommits,
            PanelId::NeuralCommits => PanelId::Preview,
            PanelId::Preview => PanelId::Status,
        }
    }

    /// Get the previous panel in the cycle.
    pub fn previous(self) -> Self {
        match self {
            PanelId::Status => PanelId::Preview,
            PanelId::Staging => PanelId::Status,
            PanelId::Files => PanelId::Staging,
            PanelId::GitCommits => PanelId::Files,
            PanelId::NeuralCommits => PanelId::GitCommits,
            PanelId::Preview => PanelId::NeuralCommits,
        }
    }
}
