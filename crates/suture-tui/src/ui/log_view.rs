//! Log view — displays commit history with ASCII branch graph.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let entries = app.log_entries();
    let cursor = app.log_cursor();

    let mut lines: Vec<Line> = Vec::new();

    if entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "No commits yet.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, entry) in entries.iter().enumerate() {
            let is_selected = i == cursor;

            // Graph column: simple vertical line + commit marker
            let graph_char = if entry.is_merge { "◆" } else { "●" };
            let graph_line = if i < entries.len() - 1 { "│" } else { " " };
            let graph_color = if is_selected {
                Color::Yellow
            } else {
                Color::DarkGray
            };

            // Branch heads
            let branch_tag = if entry.branch_heads.is_empty() {
                String::new()
            } else {
                let branches: Vec<String> = entry
                    .branch_heads
                    .iter()
                    .map(|b| format!(" [{b}]"))
                    .collect();
                branches.join("")
            };

            // Commit hash
            let hash_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            // Message
            let msg_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Build the line
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{graph_line} {graph_char} "),
                    Style::default().fg(graph_color),
                ),
                Span::styled(&entry.short_id, hash_style),
                Span::raw(" "),
                Span::styled(&entry.message, msg_style),
                Span::styled(
                    branch_tag,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            // Author + timestamp on second line
            lines.push(Line::from(vec![
                Span::styled("  │ ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}  {}", entry.author, entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    // Navigation hints
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [↑/k] Up  [↓/j] Down  [d] Diff  [PgUp/PgDn] Page  [g] Top  [G] Bottom",
        Style::default().fg(Color::DarkGray),
    )));

    let log_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Log ({} commits) ", entries.len())),
        )
        .wrap(Wrap { trim: false })
        .scroll(((cursor * 2) as u16, 0)); // 2 lines per entry

    f.render_widget(log_widget, area);
}
