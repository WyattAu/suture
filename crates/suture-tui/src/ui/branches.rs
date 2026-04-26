//! Branch management panel — list, create, delete, checkout, rename branches.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let branches = app.branch_list();
    let cursor = app.branch_cursor();
    let head_branch = app.head_branch();

    let title = format!(" Branches ({}) ", branches.len());
    let mut lines: Vec<Line> = Vec::new();

    if branches.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no branches)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let inner_height = area.height.saturating_sub(2) as usize;
        let (start, end) = super::visible_range(branches.len(), cursor, inner_height);

        for (i, (name, target)) in branches.iter().enumerate().skip(start).take(end - start) {
            let is_selected = i == cursor;
            let is_current = head_branch == Some(name.as_str());

            let prefix = if is_selected { "▶ " } else { "  " };
            let marker = if is_current { "* " } else { "  " };

            let name_style = if is_current {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let short_id = if target.len() >= 12 {
                format!("{}…", &target[..12])
            } else {
                target.clone()
            };

            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Yellow)),
                Span::styled(
                    marker,
                    if is_current {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(name, name_style),
                Span::styled(
                    format!("  {}", short_id),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        title,
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    ));

    let widget = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}
