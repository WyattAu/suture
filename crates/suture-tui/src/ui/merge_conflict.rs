//! Merge conflict resolution view.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let conflicts = app.conflict_files();
    let cursor = app.conflict_cursor();

    let title = format!(" Merge Conflicts ({}) ", conflicts.len());
    let mut lines: Vec<Line> = Vec::new();

    if conflicts.is_empty() {
        lines.push(Line::from(Span::styled(
            "No merge conflicts detected.",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Working tree is clean or all conflicts are resolved.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let inner_height = area.height.saturating_sub(2) as usize;
        let (start, end) = super::visible_range(conflicts.len(), cursor, inner_height);

        for (i, conflict) in conflicts.iter().enumerate().skip(start).take(end - start) {
            let is_selected = i == cursor;
            let prefix = if is_selected { "▶ " } else { "  " };

            let (resolution_text, resolution_style) = match conflict.resolution {
                Some(1) => ("(ours)", Style::default().fg(Color::Cyan)),
                Some(2) => ("(theirs)", Style::default().fg(Color::Magenta)),
                _ => ("(unresolved)", Style::default().fg(Color::Red)),
            };

            let path_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Yellow)),
                Span::styled(&conflict.path, path_style),
                Span::raw("  "),
                Span::styled(resolution_text, resolution_style),
            ]));
        }

        if let Some(conflict) = conflicts.get(cursor) {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("─── {} ───", conflict.path),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));

            lines.push(Line::from(Span::styled(
                "<<<<<<< HEAD (ours)",
                Style::default().fg(Color::Cyan),
            )));
            for line in conflict.ours_content.lines().take(4) {
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(Color::Cyan),
                )));
            }
            if conflict.ours_content.lines().count() > 4 {
                lines.push(Line::from(Span::styled(
                    "  ...",
                    Style::default().fg(Color::DarkGray),
                )));
            }

            lines.push(Line::from(Span::styled(
                "=======",
                Style::default().fg(Color::DarkGray),
            )));

            for line in conflict.theirs_content.lines().take(4) {
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(Color::Magenta),
                )));
            }
            if conflict.theirs_content.lines().count() > 4 {
                lines.push(Line::from(Span::styled(
                    "  ...",
                    Style::default().fg(Color::DarkGray),
                )));
            }

            lines.push(Line::from(Span::styled(
                ">>>>>>> theirs",
                Style::default().fg(Color::Magenta),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " [j/k] Navigate  [1] Ours  [2] Theirs  [m] Mark resolved  [Esc] Back",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        title,
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    ));

    let widget = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}
