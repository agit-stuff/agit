//! TUI (Terminal User Interface) module for AGIT.
//!
//! This module provides a Lazygit-style interactive terminal interface
//! for managing Git and Neural commits.

pub mod app;
pub mod ui;
pub mod event;
pub mod actions;
pub mod terminal;
pub mod keybindings;
pub mod panels;
pub mod widgets;

pub use app::App;
pub use terminal::TerminalGuard;

use crate::error::Result;

/// Main entry point for the TUI.
pub fn run() -> Result<()> {
    // Setup terminal
    let _terminal_guard = TerminalGuard::setup()?;

    // Initialize app
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    if !agit_dir.exists() {
        eprintln!("error: not an agit repository (run 'agit init' first)");
        std::process::exit(1);
    }

    let mut app = App::new(agit_dir)?;

    // Run event loop
    app.run()?;

    Ok(())
}
