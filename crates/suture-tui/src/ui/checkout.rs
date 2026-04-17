//! Checkout confirmation dialog.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let width = 60.min(area.width.saturating_sub(4));
    let height = 16.min(area.height.saturating_sub(4));

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    let dialog_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, dialog_area);

    let current = app.head_branch().unwrap_or("(detached)");
    let target = app.checkout_target().unwrap_or("?");
    let files = app.checkout_changed_files();

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Switch from ", Style::default().fg(Color::White)),
        Span::styled(
            current,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to ", Style::default().fg(Color::White)),
        Span::styled(
            target,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("?", Style::default().fg(Color::White)),
    ]));
    lines.push(Line::from(""));

    if files.is_empty() {
        lines.push(Line::from(Span::styled(
            "No file changes expected.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("Files that will change ({}):", files.len()),
            Style::default().fg(Color::Yellow),
        )));
        for file in files.iter().take(8) {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(file.as_str(), Style::default().fg(Color::White)),
            ]));
        }
        if files.len() > 8 {
            lines.push(Line::from(Span::styled(
                format!("  ... and {} more", files.len() - 8),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            " [y]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Confirm   ", Style::default().fg(Color::White)),
        Span::styled(
            "[n] / [Esc]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Cancel", Style::default().fg(Color::White)),
    ]));

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Checkout Confirmation ")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(widget, dialog_area);
}
