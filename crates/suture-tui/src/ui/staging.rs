//! Interactive staging panel — toggle files between staged and unstaged.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let focus_staged = app.staging_focus_staged();
    let cursor = app.staging_cursor();

    // Staged files pane
    let staged = app.staged_files();
    let staged_style = if focus_staged {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };

    let staged_title = format!(" Staged Files ({}) ", staged.len());
    let inner_height = chunks[0].height.saturating_sub(2) as usize; // minus border
    let (staged_start, staged_end) = super::visible_range(staged.len(), cursor, inner_height);
    let mut staged_lines: Vec<Line> = Vec::new();

    if staged.is_empty() {
        staged_lines.push(Line::from(Span::styled(
            "  (no staged files)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, entry) in staged
            .iter()
            .enumerate()
            .skip(staged_start)
            .take(staged_end - staged_start)
        {
            let is_selected = focus_staged && i == cursor;
            let icon_style = super::status_style(entry.status);
            let path_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_selected { "\u{25b6} " } else { "  " };

            staged_lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Yellow)),
                Span::styled(format!("{} ", super::status_icon(entry.status)), icon_style),
                Span::styled(&entry.path, path_style),
            ]));
        }
    }

    let staged_block = Block::default()
        .borders(Borders::ALL)
        .border_style(staged_style)
        .title(Span::styled(staged_title, staged_style));

    let staged_widget = Paragraph::new(staged_lines)
        .block(staged_block)
        .wrap(Wrap { trim: false });
    f.render_widget(staged_widget, chunks[0]);

    // Unstaged files pane
    let unstaged = app.unstaged_files();
    let unstaged_style = if focus_staged {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    };

    let unstaged_title = format!(" Unstaged Files ({}) ", unstaged.len());
    let inner_height_unstaged = chunks[1].height.saturating_sub(2) as usize;
    let (unstaged_start, unstaged_end) =
        super::visible_range(unstaged.len(), cursor, inner_height_unstaged);
    let mut unstaged_lines: Vec<Line> = Vec::new();

    if unstaged.is_empty() {
        unstaged_lines.push(Line::from(Span::styled(
            "  (no unstaged files)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, entry) in unstaged
            .iter()
            .enumerate()
            .skip(unstaged_start)
            .take(unstaged_end - unstaged_start)
        {
            let is_selected = !focus_staged && i == cursor;
            let icon_style = super::status_style(entry.status);
            let path_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_selected { "\u{25b6} " } else { "  " };

            unstaged_lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Yellow)),
                Span::styled(format!("{} ", super::status_icon(entry.status)), icon_style),
                Span::styled(&entry.path, path_style),
            ]));
        }
    }

    let unstaged_block = Block::default()
        .borders(Borders::ALL)
        .border_style(unstaged_style)
        .title(Span::styled(unstaged_title, unstaged_style));

    let unstaged_widget = Paragraph::new(unstaged_lines)
        .block(unstaged_block)
        .wrap(Wrap { trim: false });
    f.render_widget(unstaged_widget, chunks[1]);
}
