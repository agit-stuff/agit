//! Staging panel - shows AGIT index entries.

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

pub struct StagingPanel;

impl StagingPanel {
    pub fn new() -> Self {
        Self
    }
}

impl Panel for StagingPanel {
    fn id(&self) -> PanelId {
        PanelId::Staging
    }

    fn title(&self) -> &str {
        "Staging Area"
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

        let items: Vec<ListItem> = app.index_entries
            .iter()
            .map(|entry| {
                let role_color = match entry.role {
                    crate::domain::Role::User => Color::Green,
                    crate::domain::Role::Ai => Color::Blue,
                };

                let category_icon = match entry.category {
                    crate::domain::Category::Intent => "→",
                    crate::domain::Category::Reasoning => "∴",
                    crate::domain::Category::Error => "✗",
                    crate::domain::Category::Note => "•",
                };

                let line = Line::from(vec![
                    Span::styled(format!("{} ", category_icon), Style::default().fg(Color::Cyan)),
                    Span::styled(format!("[{:?}] ", entry.role), Style::default().fg(role_color)),
                    Span::raw(&entry.content),
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
