//! Log view — displays commit history with ASCII branch graph.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

use super::log_graph;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let entries = app.log_entries();
    let cursor = app.log_cursor();
    let graph_rows = log_graph::compute_graph(entries);

    let mut lines: Vec<Line> = Vec::new();

    if entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "No commits yet.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, entry) in entries.iter().enumerate() {
            let is_selected = i == cursor;
            let graph = graph_rows.get(i);

            let commit_prefix = graph.map_or_else(
                || "\u{2502} \u{25cf} ".to_owned(),
                |g| g.commit_prefix.clone(),
            );
            let info_prefix =
                graph.map_or_else(|| "  \u{2502} ".to_owned(), |g| g.info_prefix.clone());

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

            let hash_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            let msg_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let graph_color = if is_selected {
                Color::Yellow
            } else {
                Color::DarkGray
            };

            lines.push(Line::from(vec![
                Span::styled(commit_prefix, Style::default().fg(graph_color)),
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

            lines.push(Line::from(vec![
                Span::styled(info_prefix, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}  {}", entry.author, entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            if let Some(g) = graph {
                for extra in &g.extra_lines {
                    lines.push(Line::from(Span::styled(
                        extra.as_str(),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [\u{2191}/k] Up  [\u{2193}/j] Down  [d] Diff  [PgUp/PgDn] Page  [g] Top  [G] Bottom",
        Style::default().fg(Color::DarkGray),
    )));

    let mut scroll_offset = 0usize;
    for i in 0..cursor {
        scroll_offset += 2;
        if let Some(g) = graph_rows.get(i) {
            scroll_offset += g.extra_lines.len();
        }
    }

    let log_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Log ({} commits) ", entries.len())),
        )
        .wrap(Wrap { trim: false })
        .scroll((u16::try_from(scroll_offset).unwrap_or(u16::MAX), 0));

    f.render_widget(log_widget, area);
}
