//! Files panel - shows Git staged files.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::error::Result;
use crate::tui::actions::PanelId;
use crate::tui::app::App;
use super::Panel;

pub struct FilesPanel;

impl FilesPanel {
    pub fn new() -> Self {
        Self
    }
}

impl Panel for FilesPanel {
    fn id(&self) -> PanelId {
        PanelId::Files
    }

    fn title(&self) -> &str {
        "Staged Files"
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

        let items: Vec<ListItem> = app.staged_files
            .iter()
            .map(|file| {
                let line = Line::from(vec![
                    Span::styled("M ", Style::default().fg(Color::Green)),
                    Span::raw(file),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(block);

        frame.render_widget(list, area);

        Ok(())
    }
}
