//! Status panel implementation.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::error::Result;
use crate::tui::actions::PanelId;
use crate::tui::app::App;
use super::Panel;

pub struct StatusPanel;

impl StatusPanel {
    pub fn new() -> Self {
        Self
    }
}

impl Panel for StatusPanel {
    fn id(&self) -> PanelId {
        PanelId::Status
    }

    fn title(&self) -> &str {
        "Status"
    }

    fn render(&self, frame: &mut Frame, area: Rect, app: &App, _focused: bool) -> Result<()> {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title())
            .style(Style::default());

        // Build status line
        let git_branch = &app.status.git_branch;
        let agit_branch = &app.status.agit_branch;
        let in_sync = app.status.in_sync;
        let pending_count = app.index_entries.len();

        let sync_indicator = if in_sync {
            Span::styled("✓", Style::default().fg(Color::Green))
        } else {
            Span::styled("⚠ OUT OF SYNC!", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        };

        let status_line = Line::from(vec![
            Span::raw("Git Branch: "),
            Span::styled(git_branch, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" | Neural Branch: "),
            Span::styled(agit_branch, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" | Sync: "),
            sync_indicator,
            Span::raw(format!(" | Pending: {}", pending_count)),
        ]);

        let paragraph = Paragraph::new(vec![status_line])
            .block(block);

        frame.render_widget(paragraph, area);

        Ok(())
    }
}
