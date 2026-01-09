//! UI rendering logic.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::error::Result;
use crate::tui::app::App;
use crate::tui::actions::PanelId;
use crate::tui::widgets::HelpOverlay;

/// Render the entire UI.
pub fn render(frame: &mut Frame, app: &App) -> Result<()> {
    let size = frame.size();

    // Main layout: Status (top) | Content (middle)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Status panel
            Constraint::Min(0),         // Content area
        ])
        .split(size);

    // Render status panel
    let status_panel = &app.panels[0];
    status_panel.render(frame, main_chunks[0], app, app.current_panel == PanelId::Status)?;

    // Content layout: Left column | Right column
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),  // Left column (Staging + Files)
            Constraint::Percentage(60),  // Right column (Commits + Preview)
        ])
        .split(main_chunks[1]);

    // Left column: Staging (top) | Files (bottom)
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60),  // Staging area
            Constraint::Percentage(40),  // Staged files
        ])
        .split(content_chunks[0]);

    // Right column: Git Commits (top) | Neural Commits (middle) | Preview (bottom)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),  // Git commits
            Constraint::Percentage(30),  // Neural commits
            Constraint::Percentage(40),  // Preview
        ])
        .split(content_chunks[1]);

    // Render left column panels
    let staging_panel = &app.panels[1];
    staging_panel.render(frame, left_chunks[0], app, app.current_panel == PanelId::Staging)?;

    let files_panel = &app.panels[2];
    files_panel.render(frame, left_chunks[1], app, app.current_panel == PanelId::Files)?;

    // Render right column panels
    let git_commits_panel = &app.panels[3];
    git_commits_panel.render(frame, right_chunks[0], app, app.current_panel == PanelId::GitCommits)?;

    let neural_commits_panel = &app.panels[4];
    neural_commits_panel.render(frame, right_chunks[1], app, app.current_panel == PanelId::NeuralCommits)?;

    let preview_panel = &app.panels[5];
    preview_panel.render(frame, right_chunks[2], app, app.current_panel == PanelId::Preview)?;

    // Render help overlay if shown
    if app.show_help {
        HelpOverlay::render(frame);
    }

    Ok(())
}
