//! Status panel — shows repository overview.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    // Header info
    let branch = app.head_branch().unwrap_or("(detached)");
    let head = app.head_patch().map_or_else(
        || "(none)".to_owned(),
        |h| format!("{}…", &h[..12.min(h.len())]),
    );

    let header_lines = vec![
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
            Span::styled("Info:   ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{} patches, {} branches",
                app.patch_count(),
                app.branch_count()
            )),
        ]),
        Line::from(""),
    ];

    let header = Paragraph::new(header_lines)
        .block(Block::default().borders(Borders::ALL).title(" Repository "));
    f.render_widget(header, chunks[0]);

    // Quick summary
    let staged = app.staged_files();
    let unstaged = app.unstaged_files();

    let mut summary_lines = Vec::new();

    if staged.is_empty() && unstaged.is_empty() {
        summary_lines.push(Line::from(Span::styled(
            "Working tree clean.",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));
        summary_lines.push(Line::from(""));
        summary_lines.push(Line::from(Span::styled(
            "Shortcuts: [s] Staging  [l] Log  [b] Branches  [c] Commit  [r] Refresh  [q] Quit",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        if !staged.is_empty() {
            summary_lines.push(Line::from(vec![Span::styled(
                format!("Changes to be committed ({}):\n", staged.len()),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]));
            for entry in staged.iter().take(20) {
                summary_lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} ", super::status_icon(entry.status)),
                        super::status_style(entry.status),
                    ),
                    Span::raw(&entry.path),
                ]));
            }
            if staged.len() > 20 {
                summary_lines.push(Line::from(format!("  ... and {} more", staged.len() - 20)));
            }
        }

        if !unstaged.is_empty() {
            if !staged.is_empty() {
                summary_lines.push(Line::from(""));
            }
            summary_lines.push(Line::from(vec![Span::styled(
                format!("Unstaged changes ({}):\n", unstaged.len()),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]));
            for entry in unstaged.iter().take(20) {
                summary_lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} ", super::status_icon(entry.status)),
                        super::status_style(entry.status),
                    ),
                    Span::raw(&entry.path),
                ]));
            }
            if unstaged.len() > 20 {
                summary_lines.push(Line::from(format!(
                    "  ... and {} more",
                    unstaged.len() - 20
                )));
            }
        }

        summary_lines.push(Line::from(""));
        summary_lines.push(Line::from(Span::styled(
            "Shortcuts: [s] Staging  [l] Log  [b] Branches  [c] Commit  [r] Refresh  [q] Quit",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let summary = Paragraph::new(summary_lines)
        .block(Block::default().borders(Borders::ALL).title(" Summary "))
        .wrap(Wrap { trim: false });
    f.render_widget(summary, chunks[1]);
}

use ratatui::layout::Direction;
