//! Panel definitions for TUI.
//!
//! Each panel represents a distinct view in the TUI layout.

pub mod status;
pub mod staging;
pub mod files;
pub mod commits;
pub mod neural;
pub mod preview;

use ratatui::{
    layout::Rect,
    Frame,
};

use crate::error::Result;
use crate::tui::actions::PanelId;
use crate::tui::app::App;

/// Trait for TUI panels.
pub trait Panel {
    /// Get the panel's identifier.
    fn id(&self) -> PanelId;

    /// Get the panel's title.
    fn title(&self) -> &str;

    /// Render the panel.
    fn render(&self, frame: &mut Frame, area: Rect, app: &App, focused: bool) -> Result<()>;

    /// Handle panel-specific actions (optional).
    fn handle_selection(&self, _app: &App) -> Result<Option<String>> {
        Ok(None)
    }
}
