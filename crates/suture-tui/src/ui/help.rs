//! Help panel — displays keyboard shortcuts and general information.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

pub fn draw(f: &mut Frame, _app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);

    // Global
    lines.push(Line::from(Span::styled("GLOBAL", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [Tab/Shift+Tab]  ", key_style),
        Span::styled("Cycle through tabs", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Alt+1..6]      ", key_style),
        Span::styled("Jump to tab", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [q] / [Ctrl+C]  ", key_style),
        Span::styled("Quit", desc_style),
    ]));
    lines.push(Line::from(""));

    // Status tab
    lines.push(Line::from(Span::styled("STATUS TAB", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [s]  ", key_style),
        Span::styled("Go to Staging", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [l]  ", key_style),
        Span::styled("Go to Log", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [c]  ", key_style),
        Span::styled("Commit staged changes", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [r]  ", key_style),
        Span::styled("Refresh", desc_style),
    ]));
    lines.push(Line::from(""));

    // Log tab
    lines.push(Line::from(Span::styled("LOG TAB", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [↑/k]  ", key_style),
        Span::styled("Move up", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [↓/j]  ", key_style),
        Span::styled("Move down", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [d]     ", key_style),
        Span::styled("Show diff for selected commit", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [g/G]   ", key_style),
        Span::styled("Top / Bottom", desc_style),
    ]));
    lines.push(Line::from(""));

    // Staging tab
    lines.push(Line::from(Span::styled("STAGING TAB", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [↑/k]    ", key_style),
        Span::styled("Move up", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [↓/j]    ", key_style),
        Span::styled("Move down", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Space]  ", key_style),
        Span::styled("Stage / Unstage selected file", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Tab]    ", key_style),
        Span::styled("Toggle focus between Staged/Unstaged panes", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [a]      ", key_style),
        Span::styled("Stage all files", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [d]      ", key_style),
        Span::styled("Show diff for selected file", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [c]      ", key_style),
        Span::styled("Commit staged changes", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [PgUp/PgDn] ", key_style),
        Span::styled("Page up / down", desc_style),
    ]));
    lines.push(Line::from(""));

    // Branches tab
    lines.push(Line::from(Span::styled("BRANCHES TAB", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [↑/k]  ", key_style),
        Span::styled("Move up", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [↓/j]  ", key_style),
        Span::styled("Move down", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [n]    ", key_style),
        Span::styled("Create new branch", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [x]    ", key_style),
        Span::styled("Checkout selected branch", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [d]    ", key_style),
        Span::styled("Delete selected branch", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [r]    ", key_style),
        Span::styled("Rename selected branch", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [g/G]  ", key_style),
        Span::styled("Top / Bottom", desc_style),
    ]));
    lines.push(Line::from(""));

    // Commit mode
    lines.push(Line::from(Span::styled("COMMIT MODE", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [Enter]   ", key_style),
        Span::styled("Commit", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Ctrl+J]  ", key_style),
        Span::styled("Insert newline (multi-line message)", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Esc]     ", key_style),
        Span::styled("Cancel", desc_style),
    ]));
    lines.push(Line::from(""));

    // Diff tab
    lines.push(Line::from(Span::styled("DIFF TAB", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [↑/k]       ", key_style),
        Span::styled("Scroll up", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [↓/j]       ", key_style),
        Span::styled("Scroll down", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [PgUp/PgDn] ", key_style),
        Span::styled("Page up / down", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [g/G]        ", key_style),
        Span::styled("Top / Bottom", desc_style),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            "  Suture USVCS v{} — Universal Semantic Version Control",
            env!("CARGO_PKG_VERSION")
        ),
        dim_style,
    )));

    let help_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Keyboard Shortcuts "),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(help_widget, area);
}
