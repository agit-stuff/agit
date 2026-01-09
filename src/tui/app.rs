//! Application state for TUI.

use std::path::PathBuf;

use crate::core::branch_sync::BranchSync;
use crate::domain::{IndexEntry, NeuralCommit};
use crate::error::Result;
use crate::git::GitRepository;
use crate::storage::{FileHeadStore, FileIndexStore, FileObjectStore, FileRefStore, HeadStore, IndexStore, ObjectStore, RefStore};
use crate::tui::actions::{Action, PanelId};
use crate::tui::event::{Event, EventHandler};
use crate::tui::keybindings;
use crate::tui::panels::{self, Panel};
use crate::tui::terminal;
use crate::tui::ui;
use crate::domain::WrappedNeuralCommit;

/// Application state.
pub struct App {
    // Core state
    pub should_quit: bool,
    pub current_panel: PanelId,

    // Data sources
    pub agit_dir: PathBuf,
    pub git_repo: GitRepository,
    pub object_store: FileObjectStore,
    pub ref_store: FileRefStore,
    pub head_store: FileHeadStore,
    pub index_store: FileIndexStore,
    pub branch_sync: BranchSync,

    // Loaded data (cached)
    pub status: StatusData,
    pub staged_files: Vec<String>,
    pub git_commits: Vec<GitCommitSummary>,
    pub neural_commits: Vec<NeuralCommit>,
    pub index_entries: Vec<IndexEntry>,

    // UI state
    pub show_help: bool,
    pub preview_content: Option<String>,

    // Error state
    pub last_error: Option<String>,

    // Event handler
    event_handler: EventHandler,

    // Panels
    pub panels: Vec<Box<dyn Panel>>,
}

/// Status information.
pub struct StatusData {
    pub git_branch: String,
    pub agit_branch: String,
    pub in_sync: bool,
}

/// Git commit summary.
pub struct GitCommitSummary {
    pub short_hash: String,
    pub message: String,
}

impl App {
    /// Create a new application.
    pub fn new(agit_dir: PathBuf) -> Result<Self> {
        let project_root = agit_dir.parent().unwrap_or(&agit_dir).to_path_buf();

        // Initialize storage
        let git_repo = GitRepository::open(&project_root)?;
        let object_store = FileObjectStore::new(&agit_dir);
        let ref_store = FileRefStore::new(&agit_dir);
        let head_store = FileHeadStore::new(&agit_dir);
        let index_store = FileIndexStore::new(&agit_dir);
        let branch_sync = BranchSync::new(&project_root, &agit_dir)?;

        // Initialize panels
        let panels: Vec<Box<dyn Panel>> = vec![
            Box::new(panels::status::StatusPanel::new()),
            Box::new(panels::staging::StagingPanel::new()),
            Box::new(panels::files::FilesPanel::new()),
            Box::new(panels::commits::GitCommitsPanel::new()),
            Box::new(panels::neural::NeuralCommitsPanel::new()),
            Box::new(panels::preview::PreviewPanel::new()),
        ];

        let mut app = Self {
            should_quit: false,
            current_panel: PanelId::Staging,

            agit_dir: agit_dir.clone(),
            git_repo,
            object_store,
            ref_store,
            head_store,
            index_store,
            branch_sync,

            status: StatusData {
                git_branch: String::new(),
                agit_branch: String::new(),
                in_sync: false,
            },
            staged_files: Vec::new(),
            git_commits: Vec::new(),
            neural_commits: Vec::new(),
            index_entries: Vec::new(),

            show_help: false,
            preview_content: None,
            last_error: None,

            event_handler: EventHandler::default(),
            panels,
        };

        // Initial data load
        app.refresh()?;

        Ok(app)
    }

    /// Main event loop.
    pub fn run(&mut self) -> Result<()> {
        let mut terminal = terminal::create_terminal()?;

        while !self.should_quit {
            // Render
            terminal.draw(|frame| {
                if let Err(e) = ui::render(frame, self) {
                    self.last_error = Some(format!("Render error: {}", e));
                }
            })?;

            // Handle events
            if let Some(event) = tokio::runtime::Runtime::new()?.block_on(self.event_handler.next()) {
                self.handle_event(event)?;
            }
        }

        Ok(())
    }

    /// Handle an event.
    fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => {
                if let Some(action) = keybindings::handle_key_event(key, self.show_help) {
                    self.handle_action(action)?;
                }
            }
            Event::Resize(_, _) => {
                // Terminal will automatically adjust on next render
            }
            Event::Tick => {
                // Periodic refresh (optional)
            }
            Event::Refresh => {
                self.refresh()?;
            }
        }

        Ok(())
    }

    /// Handle an action.
    fn handle_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Quit => {
                self.should_quit = true;
            }
            Action::Refresh => {
                self.refresh()?;
            }
            Action::NavigateToPanel(panel_id) => {
                self.current_panel = panel_id;
            }
            Action::NextPanel => {
                self.current_panel = self.current_panel.next();
            }
            Action::PreviousPanel => {
                self.current_panel = self.current_panel.previous();
            }
            Action::ShowHelp => {
                self.show_help = true;
            }
            Action::HideHelp => {
                self.show_help = false;
            }
            // Phase 2+ actions (not implemented yet)
            Action::Commit { .. } => {
                self.last_error = Some("Commit feature not yet implemented (Phase 2)".to_string());
            }
            Action::SyncBranch => {
                self.last_error = Some("Branch sync not yet implemented (Phase 2)".to_string());
            }
            _ => {
                // Other actions not yet implemented
            }
        }

        Ok(())
    }

    /// Refresh all data from storage.
    pub fn refresh(&mut self) -> Result<()> {
        // Refresh status
        let git_branch = self.branch_sync.git_branch()?;
        let agit_branch = self.branch_sync.agit_branch()?.unwrap_or_else(|| "main".to_string());
        let sync_status = self.branch_sync.status()?;
        let in_sync = sync_status.is_in_sync();

        self.status = StatusData {
            git_branch,
            agit_branch,
            in_sync,
        };

        // Refresh index entries
        self.index_entries = self.index_store.read_all()?;

        // Refresh staged files (stub for now)
        self.staged_files = Vec::new(); // TODO: Get from git repo

        // Refresh Git commits
        self.git_commits = self.load_git_commits(10)?;

        // Refresh Neural commits
        self.neural_commits = self.load_neural_commits(10)?;

        Ok(())
    }

    /// Load Git commits.
    fn load_git_commits(&self, _count: usize) -> Result<Vec<GitCommitSummary>> {
        // Stub implementation
        // TODO: Use git2 to walk commits
        Ok(Vec::new())
    }

    /// Load Neural commits.
    fn load_neural_commits(&self, count: usize) -> Result<Vec<NeuralCommit>> {
        let branch = self.head_store.get()?.unwrap_or_else(|| "main".to_string());
        let mut current_hash = self.ref_store.get(&branch)?;
        let mut commits = Vec::new();

        while let Some(hash) = current_hash {
            if commits.len() >= count {
                break;
            }

            let data = self.object_store.load(&hash)?;
            let wrapped: WrappedNeuralCommit = serde_json::from_slice(&data)?;

            commits.push(wrapped.data.clone());
            current_hash = wrapped.data.parent_hash;
        }

        Ok(commits)
    }
}
