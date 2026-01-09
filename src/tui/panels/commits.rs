//! Git commits panel.

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

pub struct GitCommitsPanel;

impl GitCommitsPanel {
    pub fn new() -> Self {
        Self
    }
}

impl Panel for GitCommitsPanel {
    fn id(&self) -> PanelId {
        PanelId::GitCommits
    }

    fn title(&self) -> &str {
        "Git Commits"
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

        let items: Vec<ListItem> = app.git_commits
            .iter()
            .map(|commit| {
                let line = Line::from(vec![
                    Span::styled(&commit.short_hash, Style::default().fg(Color::Yellow)),
                    Span::raw(" - "),
                    Span::raw(&commit.message),
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
