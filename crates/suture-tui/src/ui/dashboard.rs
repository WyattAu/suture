//! Dashboard view — repo overview with recent patches, working set status, quick actions.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(area);

    draw_repo_info(f, app, chunks[0]);
    draw_recent_patches(f, app, chunks[1]);
    draw_quick_actions(f, app, chunks[2]);
}

fn draw_repo_info(f: &mut Frame, app: &App, area: Rect) {
    let branch = app.head_branch().unwrap_or("(detached)");
    let head = app
        .head_patch().map_or_else(|| "(none)".to_owned(), |h| format!("{}…", &h[..12.min(h.len())]));

    let working_set_dirty = !app.unstaged_files().is_empty() || !app.staged_files().is_empty();
    let working_set_label = if working_set_dirty { "dirty" } else { "clean" };
    let working_set_style = if working_set_dirty {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    };

    let repo_name = app
        .repo()
        .root()
        .file_name().map_or_else(|| "unknown".to_owned(), |n| n.to_string_lossy().to_string());

    let has_conflicts = !app.conflict_files().is_empty();

    let lines = vec![
        Line::from(vec![
            Span::styled("Repo:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                repo_name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Branch: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                branch,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Head:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(&head, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(working_set_label, working_set_style),
            if has_conflicts {
                Span::styled(" | conflicts", Style::default().fg(Color::Red))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::styled("Info:   ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{} patches, {} branches",
                app.patch_count(),
                app.branch_count()
            )),
        ]),
    ];

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Repository Info "),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}

fn draw_recent_patches(f: &mut Frame, app: &App, area: Rect) {
    let entries = app.log_entries();
    let max_display = (area.height.saturating_sub(2) as usize).min(entries.len());

    let mut lines: Vec<Line> = Vec::new();

    if entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No patches yet.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for entry in entries.iter().take(max_display) {
            let merge_tag = if entry.is_merge {
                Span::styled(" \u{25c6}", Style::default().fg(Color::Magenta))
            } else {
                Span::raw("")
            };
            lines.push(Line::from(vec![
                Span::styled(&entry.short_id, Style::default().fg(Color::Cyan)),
                merge_tag,
                Span::raw(" "),
                Span::styled(&entry.message, Style::default().fg(Color::White)),
                Span::raw("  "),
                Span::styled(
                    &entry.timestamp,
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        if entries.len() > max_display {
            lines.push(Line::from(Span::styled(
                format!("  ... and {} more (press [l] for full log)", entries.len() - max_display),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Recent Patches ({}) ", entries.len())),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}

fn draw_quick_actions(f: &mut Frame, _app: &App, area: Rect) {
    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let sep_style = Style::default().fg(Color::DarkGray);

    let lines = vec![Line::from(vec![
        Span::styled(" [n]", key_style),
        Span::styled(" New Patch ", desc_style),
        Span::styled("\u{2502}", sep_style),
        Span::styled(" [c]", key_style),
        Span::styled(" Commit ", desc_style),
        Span::styled("\u{2502}", sep_style),
        Span::styled(" [s]", key_style),
        Span::styled(" Stage ", desc_style),
        Span::styled("\u{2502}", sep_style),
        Span::styled(" [l]", key_style),
        Span::styled(" Log ", desc_style),
        Span::styled("\u{2502}", sep_style),
        Span::styled(" [b]", key_style),
        Span::styled(" Branches ", desc_style),
        Span::styled("\u{2502}", sep_style),
        Span::styled(" [r]", key_style),
        Span::styled(" Remote ", desc_style),
    ])];

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Quick Actions "),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}
