//! Help overlay widget.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub struct HelpOverlay;

impl HelpOverlay {
    /// Render the help overlay.
    pub fn render(frame: &mut Frame) {
        let area = Self::centered_rect(60, 70, frame.size());

        // Clear the background
        frame.render_widget(Clear, area);

        // Create help content
        let help_text = vec![
            Line::from(vec![
                Span::styled("AGIT TUI - Keyboard Shortcuts", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Global:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  q             Quit"),
            Line::from("  ?             Show/hide this help"),
            Line::from("  1-6           Jump to panel (1=Status, 2=Staging, 3=Files, 4=Git, 5=Neural, 6=Preview)"),
            Line::from("  Tab           Next panel"),
            Line::from("  Shift+Tab     Previous panel"),
            Line::from("  r             Refresh all data"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Navigation:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  j / Down      Move down"),
            Line::from("  k / Up        Move up"),
            Line::from("  g             Go to top"),
            Line::from("  G             Go to bottom"),
            Line::from("  Enter         View in preview"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Actions (Phase 2+):", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  c             Commit (with message dialog)"),
            Line::from("  S             Sync AGIT branch to Git branch"),
            Line::from("  Space         Stage/unstage file"),
            Line::from("  a             Stage all"),
            Line::from("  u             Unstage all"),
            Line::from("  d             Delete index entry"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Press ? or Esc to close", Style::default().fg(Color::Green)),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
            .style(Style::default().bg(Color::Black).fg(Color::White));

        let paragraph = Paragraph::new(help_text)
            .block(block)
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, area);
    }

    /// Create a centered rectangle.
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}
