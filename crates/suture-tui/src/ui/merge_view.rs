//! Merge visualization — 3-way merge view showing base/ours/theirs/result.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let conflicts = app.conflict_files();
    let cursor = app.conflict_cursor();

    if conflicts.is_empty() {
        draw_no_conflicts(f, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    draw_merge_header(f, app, cursor, chunks[0]);
    draw_three_way(f, app, cursor, chunks[1]);
    draw_merge_result(f, app, cursor, chunks[2]);
}

fn draw_no_conflicts(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "No merge conflicts to visualize.",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "All conflicts are resolved or none exist.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "To start a merge, use: suture merge <branch>",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 3-Way Merge View "),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}

fn draw_merge_header(f: &mut Frame, app: &App, cursor: usize, area: Rect) {
    let conflicts = app.conflict_files();
    let conflict = &conflicts[cursor];

    let ours_label = app.head_branch().unwrap_or("ours");
    let theirs_label = "theirs";

    let lines = vec![Line::from(vec![
        Span::styled(
            format!(" {} ", conflict.path),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("Base", Style::default().fg(Color::DarkGray)),
        Span::raw(" \u{2192} "),
        Span::styled(
            format!("Ours({ours_label})"),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(" + "),
        Span::styled(theirs_label, Style::default().fg(Color::Magenta)),
        Span::raw(" \u{2192} "),
        Span::styled("Result", Style::default().fg(Color::Green)),
    ])];

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Merge [{}/{}] ", cursor + 1, conflicts.len())),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}

fn draw_three_way(f: &mut Frame, app: &App, cursor: usize, area: Rect) {
    let conflicts = app.conflict_files();
    let conflict = &conflicts[cursor];

    let inner = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_side_panel(f, " Ours ", &conflict.hunks, true, inner[0]);
    draw_side_panel(f, " Theirs ", &conflict.hunks, false, inner[1]);
}

fn draw_side_panel(
    f: &mut Frame,
    title: &str,
    hunks: &[crate::app::Hunk],
    is_ours: bool,
    area: Rect,
) {
    let mut lines: Vec<Line> = Vec::new();

    if hunks.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (empty)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (idx, hunk) in hunks.iter().enumerate() {
            let side_lines = if is_ours { &hunk.ours_lines } else { &hunk.theirs_lines };
            let color = if is_ours { Color::Cyan } else { Color::Magenta };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  Hunk {} ", idx + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("({} lines)", side_lines.len()),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            let max_lines = 8usize;
            for line in side_lines.iter().take(max_lines) {
                lines.push(Line::from(vec![
                    Span::styled("  \u{2502} ", Style::default().fg(Color::DarkGray)),
                    Span::styled(line.as_str(), Style::default().fg(color)),
                ]));
            }
            if side_lines.len() > max_lines {
                lines.push(Line::from(Span::styled(
                    format!("  │ ... ({} more)", side_lines.len() - max_lines),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            if idx < hunks.len() - 1 {
                lines.push(Line::from(""));
            }
        }
    }

    let border_color = if is_ours { Color::Cyan } else { Color::Magenta };

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .border_style(Style::default().fg(border_color)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}

fn draw_merge_result(f: &mut Frame, app: &App, cursor: usize, area: Rect) {
    let conflicts = app.conflict_files();
    let conflict = &conflicts[cursor];

    let mut lines: Vec<Line> = Vec::new();

    let resolved = conflict
        .hunks
        .iter()
        .filter(|h| h.resolution != crate::app::HunkResolution::Unresolved)
        .count();
    let total = conflict.hunks.len();
    let all_resolved = resolved == total && total > 0;

    if all_resolved {
        lines.push(Line::from(Span::styled(
            format!(" All {total} hunks resolved "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!(" {resolved}/{total} hunks resolved "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));

    for (idx, hunk) in conflict.hunks.iter().enumerate() {
        let resolution_label = match hunk.resolution {
            crate::app::HunkResolution::Unresolved => "CONFLICT",
            crate::app::HunkResolution::Ours => "ours",
            crate::app::HunkResolution::Theirs => "theirs",
            crate::app::HunkResolution::Both => "both",
        };
        let resolution_color = match hunk.resolution {
            crate::app::HunkResolution::Unresolved => Color::Red,
            crate::app::HunkResolution::Ours => Color::Cyan,
            crate::app::HunkResolution::Theirs => Color::Magenta,
            crate::app::HunkResolution::Both => Color::Green,
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  Hunk {}: ", idx + 1),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                resolution_label,
                Style::default()
                    .fg(resolution_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [j/k] Next/Prev file  [e] Open in editor  [r] Rescan  [Esc] Back",
        Style::default().fg(Color::DarkGray),
    )));

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Merge Result ")
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}
