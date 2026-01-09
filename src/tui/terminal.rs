//! Terminal management for TUI.
//!
//! Handles terminal setup, teardown, and cleanup using RAII pattern.

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Stdout};

use crate::error::Result;

/// RAII guard for terminal state.
///
/// Automatically restores the terminal to its original state on drop,
/// even in case of panic.
pub struct TerminalGuard {
    _guard: (),
}

impl TerminalGuard {
    /// Setup the terminal for TUI mode.
    pub fn setup() -> Result<Self> {
        // Enable raw mode
        enable_raw_mode()?;

        // Enter alternate screen and enable mouse capture
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture
        )?;

        Ok(Self { _guard: () })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Restore terminal state
        // Ignore errors during cleanup
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
    }
}

/// Create a terminal backend for Ratatui.
pub fn create_terminal() -> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<Stdout>>> {
    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let terminal = ratatui::Terminal::new(backend)?;
    Ok(terminal)
}
