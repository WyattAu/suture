//! Remote management panel.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let remotes = app.remote_list();
    let cursor = app.remote_cursor();

    let title = format!(" Remotes ({}) ", remotes.len());
    let mut lines: Vec<Line> = Vec::new();

    if remotes.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no remotes configured)",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Remotes allow you to sync with a Suture Hub.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "Use [a] to add a remote.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let inner_height = area.height.saturating_sub(2) as usize;
        let (start, end) = super::visible_range(remotes.len(), cursor, inner_height);

        for (i, (name, url)) in remotes.iter().enumerate().skip(start).take(end - start) {
            let is_selected = i == cursor;
            let prefix = if is_selected { "\u{25b6} " } else { "  " };

            let name_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Yellow)),
                Span::styled(name, name_style),
                Span::raw("  "),
                Span::styled(url, Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [a] Add remote  [d] Remove selected  [\u{2191}/k] Up  [\u{2193}/j] Down",
        Style::default().fg(Color::DarkGray),
    )));

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
