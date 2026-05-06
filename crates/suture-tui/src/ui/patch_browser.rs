//! Patch browser — list, inspect, search, and filter patches.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let entries = app.log_entries();
    let cursor = app.patch_browser_cursor();
    let filter = app.patch_browser_filter();
    let sort_desc = app.patch_browser_sort_desc();

    let filtered: Vec<_> = if filter.is_empty() {
        entries.iter().collect()
    } else {
        let filter_lower = filter.to_lowercase();
        entries
            .iter()
            .filter(|e| {
                e.message.to_lowercase().contains(&filter_lower)
                    || e.author.to_lowercase().contains(&filter_lower)
                    || e.id.to_lowercase().contains(&filter_lower)
            })
            .collect()
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    draw_search_bar(f, app, chunks[0]);
    draw_patch_list(f, &filtered, cursor, sort_desc, chunks[1]);
}

fn draw_search_bar(f: &mut Frame, app: &App, area: Rect) {
    let filter = app.patch_browser_filter();
    let total = app.log_entries().len();

    let lines = vec![Line::from(vec![
        Span::styled("/", Style::default().fg(Color::Yellow)),
        Span::styled(
            if filter.is_empty() {
                "Search patches (type to filter)...".to_owned()
            } else {
                filter.to_owned()
            },
            if filter.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            },
        ),
        Span::styled(
            format!("  [{total} patches]"),
            Style::default().fg(Color::DarkGray),
        ),
    ])];

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Patch Browser "),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}

fn draw_patch_list(
    f: &mut Frame,
    patches: &[&crate::app::LogEntry],
    cursor: usize,
    sort_desc: bool,
    area: Rect,
) {
    let inner_height = area.height.saturating_sub(2) as usize;
    let (start, end) = super::visible_range(patches.len(), cursor, inner_height);

    let mut lines: Vec<Line> = Vec::new();

    if patches.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No patches match the filter.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, entry) in patches.iter().enumerate().skip(start).take(end - start) {
            let is_selected = i == cursor;
            let prefix = if is_selected { "\u{25b6} " } else { "  " };

            let merge_icon = if entry.is_merge { "\u{25c6} " } else { "  " };
            let merge_style = if entry.is_merge {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default()
            };

            let branch_tags = if entry.branch_heads.is_empty() {
                String::new()
            } else {
                let tags: Vec<String> = entry
                    .branch_heads
                    .iter()
                    .map(|b| format!("[{b}]"))
                    .collect();
                format!(" {}", tags.join(""))
            };

            let id_style = if is_selected {
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

            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Yellow)),
                Span::styled(merge_icon, merge_style),
                Span::styled(&entry.short_id, id_style),
                Span::styled(" ", Style::default()),
                Span::styled(&entry.message, msg_style),
                Span::styled(
                    branch_tags,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("       ", Style::default()),
                Span::styled(
                    format!("{}  {}", entry.author, entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                if entry.is_merge {
                    Span::styled(
                        format!("  parents: {}", entry.parents.len()),
                        Style::default().fg(Color::DarkGray),
                    )
                } else {
                    Span::raw("")
                },
            ]));
        }
    }

    lines.push(Line::from(""));
    let sort_label = if sort_desc {
        "newest first"
    } else {
        "oldest first"
    };
    lines.push(Line::from(Span::styled(
        format!(
            " [↑/k] Up  [↓/j] Down  [d] Diff  [Enter] Details  [s] Sort ({sort_label})  [Esc] Clear filter"
        ),
        Style::default().fg(Color::DarkGray),
    )));

    let widget = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Patches ({}/{}) ",
            patches.len(),
            cursor + 1
        )))
        .wrap(Wrap { trim: false });
    f.render_widget(widget, area);
}
