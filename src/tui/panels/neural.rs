//! Neural commits panel.

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

pub struct NeuralCommitsPanel;

impl NeuralCommitsPanel {
    pub fn new() -> Self {
        Self
    }
}

impl Panel for NeuralCommitsPanel {
    fn id(&self) -> PanelId {
        PanelId::NeuralCommits
    }

    fn title(&self) -> &str {
        "Neural Commits"
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

        let items: Vec<ListItem> = app.neural_commits
            .iter()
            .map(|commit| {
                let short_hash = &commit.git_hash[..7.min(commit.git_hash.len())];
                let line = Line::from(vec![
                    Span::styled(short_hash, Style::default().fg(Color::Magenta)),
                    Span::raw(" - "),
                    Span::raw(&commit.summary),
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
