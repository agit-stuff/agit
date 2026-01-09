//! Preview panel - shows details of selected item.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::error::Result;
use crate::tui::actions::PanelId;
use crate::tui::app::App;
use super::Panel;

pub struct PreviewPanel;

impl PreviewPanel {
    pub fn new() -> Self {
        Self
    }
}

impl Panel for PreviewPanel {
    fn id(&self) -> PanelId {
        PanelId::Preview
    }

    fn title(&self) -> &str {
        "Preview"
    }

    fn render(&self, frame: &mut Frame, area: Rect, app: &App, focused: bool) -> Result<()> {
        let title = if focused {
            format!("[{}]", self.title())
        } else {
            self.title().to_string()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(if focused {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            });

        let content = if let Some(preview) = &app.preview_content {
            preview.clone()
        } else {
            "Select an item to preview".to_string()
        };

        let paragraph = Paragraph::new(content)
            .block(block);

        frame.render_widget(paragraph, area);

        Ok(())
    }
}
