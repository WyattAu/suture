//! Merge conflict resolution view — conflict browser with $EDITOR integration.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, ConflictFileState, HunkResolution};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let conflicts = app.conflict_files();
    let cursor = app.conflict_cursor();

    let title = format!(" Merge Conflicts ({}) ", conflicts.len());

    if conflicts.is_empty() {
        let lines = vec![
            Line::from(Span::styled(
                "No merge conflicts detected.",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Working tree is clean or all conflicts are resolved.",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            title,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        let widget = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });
        f.render_widget(widget, area);
        return;
    }

    let inner_height = area.height.saturating_sub(4) as usize;
    let (file_start, file_end) = super::visible_range(conflicts.len(), cursor, inner_height);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    // File list
    let mut file_lines: Vec<Line> = Vec::new();
    for (i, cf) in conflicts
        .iter()
        .enumerate()
        .skip(file_start)
        .take(file_end - file_start)
    {
        let is_selected = i == cursor;
        let prefix = if is_selected { "▶ " } else { "  " };
        let resolved_count = cf
            .hunks
            .iter()
            .filter(|h| h.resolution != HunkResolution::Unresolved)
            .count();
        let total = cf.hunks.len();
        let label: &str = if resolved_count == total && total > 0 {
            "resolved"
        } else {
            "conflict"
        };
        let style = if resolved_count == total && total > 0 {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };
        let count_text = format!(" {} hunks", total);
        let path_style = if is_selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        file_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Yellow)),
            Span::styled(&cf.path, path_style),
            Span::raw("  "),
            Span::styled(label, style),
            Span::styled(count_text, Style::default().fg(Color::DarkGray)),
        ]));
    }
    let file_list = Paragraph::new(file_lines)
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            format!(" Conflict Files [{}/{}] ", cursor + 1, conflicts.len()),
            Style::default().fg(Color::Cyan),
        )))
        .wrap(Wrap { trim: false });
    f.render_widget(file_list, chunks[0]);

    // Detail panel — show conflict preview for selected file
    let conflict = &conflicts[cursor];
    let detail_lines = render_conflict_preview(conflict);
    let detail_widget = Paragraph::new(detail_lines)
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                format!(" {} ", conflict.path),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(detail_widget, chunks[1]);

    // Footer with key bindings
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " e/Enter:open in editor ",
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(" j/k:navigate ", Style::default().fg(Color::DarkGray)),
        Span::styled(" r:rescan ", Style::default().fg(Color::DarkGray)),
        Span::styled(" Esc:back ", Style::default().fg(Color::DarkGray)),
    ]))
    .style(Style::default().bg(Color::DarkGray));
    f.render_widget(footer, chunks[2]);
}

fn render_conflict_preview(conflict: &ConflictFileState) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    if conflict.hunks.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no conflict hunks — file may already be resolved)",
            Style::default().fg(Color::DarkGray),
        )));
        return lines;
    }

    let total = conflict.hunks.len();
    let resolved_count = conflict
        .hunks
        .iter()
        .filter(|h| h.resolution != HunkResolution::Unresolved)
        .count();

    lines.push(Line::from(vec![
        Span::styled(
            format!(" {} conflict hunk(s) ", total),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} resolved", resolved_count),
            Style::default().fg(if resolved_count == total {
                Color::Green
            } else {
                Color::Yellow
            }),
        ),
    ]));
    lines.push(Line::from(""));

    // Show first 3 hunks as a preview
    let max_preview = 3usize;
    for (idx, hunk) in conflict.hunks.iter().take(max_preview).enumerate() {
        let status = match hunk.resolution {
            HunkResolution::Unresolved => "✗",
            _ => "✓",
        };
        let status_color = match hunk.resolution {
            HunkResolution::Unresolved => Color::Red,
            _ => Color::Green,
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  Hunk {}: ", idx + 1),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(status, Style::default().fg(status_color)),
            Span::raw(" "),
            Span::styled(
                format!("ours: {} lines", hunk.ours_lines.len()),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("  "),
            Span::styled(
                format!("theirs: {} lines", hunk.theirs_lines.len()),
                Style::default().fg(Color::Magenta),
            ),
        ]));

        // Show first 2 lines of each side
        let preview_count = 2usize;
        for line in hunk.ours_lines.iter().take(preview_count) {
            lines.push(Line::from(Span::styled(
                format!("    │ {}", line),
                Style::default().fg(Color::Cyan),
            )));
        }
        if hunk.ours_lines.len() > preview_count {
            lines.push(Line::from(Span::styled(
                format!("    │ ... ({} more)", hunk.ours_lines.len() - preview_count),
                Style::default().fg(Color::DarkGray),
            )));
        }
        lines.push(Line::from(Span::styled(
            "    │ =======",
            Style::default().fg(Color::DarkGray),
        )));
        for line in hunk.theirs_lines.iter().take(preview_count) {
            lines.push(Line::from(Span::styled(
                format!("    │ {}", line),
                Style::default().fg(Color::Magenta),
            )));
        }
        if hunk.theirs_lines.len() > preview_count {
            lines.push(Line::from(Span::styled(
                format!(
                    "    │ ... ({} more)",
                    hunk.theirs_lines.len() - preview_count
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }
        lines.push(Line::from(""));
    }

    if total > max_preview {
        lines.push(Line::from(Span::styled(
            format!(
                "  ... and {} more hunk(s). Press 'e' to open in editor.",
                total - max_preview
            ),
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines
}
