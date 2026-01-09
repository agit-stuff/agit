//! Keyboard bindings for TUI.
//!
//! Maps keyboard events to actions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::actions::{Action, PanelId};

/// Handle a key event and return the corresponding action.
pub fn handle_key_event(key: KeyEvent, show_help: bool) -> Option<Action> {
    // Help overlay takes precedence
    if show_help {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                return Some(Action::HideHelp);
            }
            _ => return None,
        }
    }

    // Global keybindings
    match key.code {
        // Quit
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),

        // Help
        KeyCode::Char('?') => Some(Action::ShowHelp),

        // Panel navigation (numeric)
        KeyCode::Char('1') => Some(Action::NavigateToPanel(PanelId::Status)),
        KeyCode::Char('2') => Some(Action::NavigateToPanel(PanelId::Staging)),
        KeyCode::Char('3') => Some(Action::NavigateToPanel(PanelId::Files)),
        KeyCode::Char('4') => Some(Action::NavigateToPanel(PanelId::GitCommits)),
        KeyCode::Char('5') => Some(Action::NavigateToPanel(PanelId::NeuralCommits)),
        KeyCode::Char('6') => Some(Action::NavigateToPanel(PanelId::Preview)),

        // Panel navigation (Tab)
        KeyCode::Tab => Some(Action::NextPanel),
        KeyCode::BackTab => Some(Action::PreviousPanel),

        // List navigation (vim-style)
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Char('g') => Some(Action::MoveToTop),
        KeyCode::Char('G') => Some(Action::MoveToBottom),

        // Refresh
        KeyCode::Char('r') => Some(Action::Refresh),

        // View in preview
        KeyCode::Enter => Some(Action::ViewInPreview),

        // Sync branch (Phase 2)
        KeyCode::Char('S') => Some(Action::SyncBranch),

        // Staging operations (Phase 3)
        KeyCode::Char(' ') => Some(Action::StageFile { path: String::new() }), // Will be filled by panel
        KeyCode::Char('a') => Some(Action::StageAll),
        KeyCode::Char('u') => Some(Action::UnstageAll),
        KeyCode::Char('d') => Some(Action::DeleteIndexEntry { index: 0 }), // Will be filled by panel

        _ => None,
    }
}
