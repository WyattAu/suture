//! Diff viewer — displays file diffs with syntax highlighting.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{App, DiffLineType};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let diff_lines = app.diff_lines();
    let file_name = app.diff_file().unwrap_or("(no diff)");

    let mut lines: Vec<Line> = Vec::new();

    if diff_lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No diff to display. Select a file in Staging or a commit in Log.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for line in diff_lines {
            match line.line_type {
                DiffLineType::Context => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{:>4} ", line.old_line.unwrap_or(0)),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("{:>4} ", line.new_line.unwrap_or(0)),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(" ", Style::default()),
                        Span::styled(&line.content, super::diff_line_style(line.line_type)),
                    ]));
                }
                DiffLineType::Add => {
                    lines.push(Line::from(vec![
                        Span::styled("     ", Style::default()),
                        Span::styled(
                            format!("{:>4} ", line.new_line.unwrap_or(0)),
                            Style::default().fg(Color::Green),
                        ),
                        Span::styled("+", Style::default().fg(Color::Green)),
                        Span::styled(&line.content, super::diff_line_style(line.line_type)),
                    ]));
                }
                DiffLineType::Remove => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{:>4} ", line.old_line.unwrap_or(0)),
                            Style::default().fg(Color::Red),
                        ),
                        Span::styled("     ", Style::default()),
                        Span::styled("-", Style::default().fg(Color::Red)),
                        Span::styled(&line.content, super::diff_line_style(line.line_type)),
                    ]));
                }
                DiffLineType::HunkHeader => {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", &line.content),
                        super::diff_line_style(line.line_type),
                    )));
                }
                DiffLineType::ConflictMarker => {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", &line.content),
                        super::diff_line_style(line.line_type),
                    )));
                }
            }
        }
    }

    // Navigation hint
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [↑/k] Scroll Up  [↓/j] Scroll Down  [PgUp/PgDn] Page  [g] Top  [G] Bottom",
        Style::default().fg(Color::DarkGray),
    )));

    let scroll = app.diff_scroll();
    let scroll_u16 = scroll as u16;

    let diff_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Diff: {file_name} ")),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll_u16, 0));

    f.render_widget(diff_widget, area);
}
