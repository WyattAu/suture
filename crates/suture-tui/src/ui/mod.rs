//! UI rendering for the Suture TUI.

mod branches;
mod diff_view;
mod help;
mod log_view;
mod staging;
mod status;

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};
use ratatui::Frame;

use crate::app::{App, DiffLineType, Tab};

/// Main draw function — renders the entire TUI.
pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();

    // Top: tab bar
    let tab_titles: Vec<Line> = Tab::ALL
        .iter()
        .map(|t| {
            let style = if *t == app.current_tab() {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(t.title(), style))
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).title(" Suture "))
        .select(
            Tab::ALL
                .iter()
                .position(|&t| t == app.current_tab())
                .unwrap_or(0),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    let tab_area = Rect {
        x: size.x,
        y: size.y,
        width: size.width,
        height: 3,
    };
    f.render_widget(tabs, tab_area);

    // Middle: main content
    let content_area = Rect {
        x: size.x,
        y: size.y + 3,
        width: size.width,
        height: size.height - 5,
    };

    match app.current_tab() {
        Tab::Status => status::draw(f, app, content_area),
        Tab::Log => log_view::draw(f, app, content_area),
        Tab::Staging => staging::draw(f, app, content_area),
        Tab::Diff => diff_view::draw(f, app, content_area),
        Tab::Branches => branches::draw(f, app, content_area),
        Tab::Help => help::draw(f, app, content_area),
    }

    // Bottom: status bar
    let status_area = Rect {
        x: size.x,
        y: size.y + size.height - 2,
        width: size.width,
        height: 2,
    };

    let status_text = if let Some(err) = app.error_message() {
        Line::from(Span::styled(
            format!(" ERROR: {err} "),
            Style::default().fg(Color::White).bg(Color::Red),
        ))
    } else if app.commit_mode() {
        // Show commit message (truncate for status bar, replacing newlines with ↵)
        let display_msg = app.commit_message().replace('\n', "↵");
        let truncated = if display_msg.len() > 40 {
            format!("{}…", &display_msg[..40])
        } else {
            display_msg
        };
        let msg = format!(" Commit: {}█ ", truncated);
        Line::from(Span::styled(
            msg,
            Style::default().fg(Color::Black).bg(Color::Green),
        ))
    } else if app.branch_input_mode() {
        let msg = format!(" Branch: {}█ ", app.branch_input());
        Line::from(Span::styled(
            msg,
            Style::default().fg(Color::Black).bg(Color::Magenta),
        ))
    } else {
        let branch = app.head_branch().unwrap_or("HEAD");
        let staged = app.staged_files().len();
        let unstaged = app.unstaged_files().len();
        let msg = app.status_message();
        let suffix = if msg.is_empty() {
            String::new()
        } else {
            format!(" | {msg}")
        };
        Line::from(vec![
            Span::styled(
                format!(" {branch} "),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                " staged:{} unstaged:{} patches:{} branches:{}{suffix} ",
                staged,
                unstaged,
                app.patch_count(),
                app.branch_count()
            )),
        ])
    };

    let status_bar = Paragraph::new(status_text).style(Style::default().bg(Color::DarkGray));
    f.render_widget(status_bar, status_area);
}

/// Render a file status icon.
pub fn status_icon(status: suture_common::FileStatus) -> &'static str {
    match status {
        suture_common::FileStatus::Added => "A",
        suture_common::FileStatus::Modified => "M",
        suture_common::FileStatus::Deleted => "D",
        suture_common::FileStatus::Clean => " ",
        suture_common::FileStatus::Untracked => "?",
    }
}

/// Style for a file status icon.
pub fn status_style(status: suture_common::FileStatus) -> Style {
    match status {
        suture_common::FileStatus::Added => Style::default().fg(Color::Green),
        suture_common::FileStatus::Modified => Style::default().fg(Color::Yellow),
        suture_common::FileStatus::Deleted => Style::default().fg(Color::Red),
        suture_common::FileStatus::Clean => Style::default().fg(Color::DarkGray),
        suture_common::FileStatus::Untracked => Style::default().fg(Color::Blue),
    }
}

/// Style for a diff line.
pub fn diff_line_style(line_type: DiffLineType) -> Style {
    match line_type {
        DiffLineType::Context => Style::default().fg(Color::Gray),
        DiffLineType::Add => Style::default().fg(Color::Green),
        DiffLineType::Remove => Style::default().fg(Color::Red),
        DiffLineType::HunkHeader => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        DiffLineType::ConflictMarker => Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    }
}

/// Compute visible range for a scrollable list.
pub fn visible_range(total: usize, scroll: usize, height: usize) -> (usize, usize) {
    let start = scroll.min(total.saturating_sub(height));
    let end = (start + height).min(total);
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_range_normal() {
        assert_eq!(visible_range(100, 10, 20), (10, 30));
    }

    #[test]
    fn test_visible_range_clamped() {
        assert_eq!(visible_range(10, 50, 20), (0, 10));
    }

    #[test]
    fn test_status_icon_added() {
        assert_eq!(status_icon(suture_common::FileStatus::Added), "A");
    }

    #[test]
    fn test_status_icon_modified() {
        assert_eq!(status_icon(suture_common::FileStatus::Modified), "M");
    }

    #[test]
    fn test_status_icon_deleted() {
        assert_eq!(status_icon(suture_common::FileStatus::Deleted), "D");
    }

    #[test]
    fn test_status_icon_untracked() {
        assert_eq!(status_icon(suture_common::FileStatus::Untracked), "?");
    }

    #[test]
    fn test_status_icon_clean() {
        assert_eq!(status_icon(suture_common::FileStatus::Clean), " ");
    }
}
