//! Help panel — displays keyboard shortcuts and general information.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

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
        Span::styled("  [Alt+1..0]       ", key_style),
        Span::styled("Jump to tab (1=Dashboard, 2=Status, ...)", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [q] / [Ctrl+C]  ", key_style),
        Span::styled("Quit", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [?]             ", key_style),
        Span::styled("Show this help", desc_style),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "QUICK REFERENCE",
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("  [1]  ", key_style),
        Span::styled("Dashboard", desc_style),
        Span::raw("   "),
        Span::styled("[2]  ", key_style),
        Span::styled("Status", desc_style),
        Span::raw("      "),
        Span::styled("[3]  ", key_style),
        Span::styled("Log", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [4]  ", key_style),
        Span::styled("Staging", desc_style),
        Span::raw("   "),
        Span::styled("[5]  ", key_style),
        Span::styled("Diff", desc_style),
        Span::raw("       "),
        Span::styled("[6]  ", key_style),
        Span::styled("Patches", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [7]  ", key_style),
        Span::styled("Merge View", desc_style),
        Span::raw("  "),
        Span::styled("[8]  ", key_style),
        Span::styled("Branches", desc_style),
        Span::raw("  "),
        Span::styled("[9]  ", key_style),
        Span::styled("Remote", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [0]  ", key_style),
        Span::styled("Help", desc_style),
    ]));
    lines.push(Line::from(""));

    // Navigation
    lines.push(Line::from(Span::styled("NAVIGATION", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [j/↓]    ", key_style),
        Span::styled("Move down / scroll down", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [k/↑]    ", key_style),
        Span::styled("Move up / scroll up", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [g/G]    ", key_style),
        Span::styled("Scroll to top / bottom", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [PgUp/PgDn] ", key_style),
        Span::styled("Page up / down", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Tab]        ", key_style),
        Span::styled("Next panel", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Shift+Tab]  ", key_style),
        Span::styled("Previous panel", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [/]          ", key_style),
        Span::styled("Search / filter", desc_style),
    ]));
    lines.push(Line::from(""));

    // Dashboard tab
    lines.push(Line::from(Span::styled("DASHBOARD TAB", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [n]  ", key_style),
        Span::styled("New patch (commit)", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [c]  ", key_style),
        Span::styled("Commit staged changes", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [s]  ", key_style),
        Span::styled("Go to Staging", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [l]  ", key_style),
        Span::styled("Go to Log", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [b]  ", key_style),
        Span::styled("Go to Branches", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [p]  ", key_style),
        Span::styled("Go to Patch Browser", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [m]  ", key_style),
        Span::styled("Go to Merge View", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [r]  ", key_style),
        Span::styled("Go to Remote", desc_style),
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
        Span::styled("Checkout selected branch (with confirmation)", desc_style),
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

    // Patch Browser tab
    lines.push(Line::from(Span::styled("PATCH BROWSER TAB", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [↑/k]     ", key_style),
        Span::styled("Move up", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [↓/j]     ", key_style),
        Span::styled("Move down", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [d/Enter] ", key_style),
        Span::styled("Show diff for selected patch", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [s]       ", key_style),
        Span::styled("Toggle sort order", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [/]       ", key_style),
        Span::styled("Search / filter patches", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Esc]     ", key_style),
        Span::styled("Clear filter", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [g/G]     ", key_style),
        Span::styled("Top / Bottom", desc_style),
    ]));
    lines.push(Line::from(""));

    // Merge View tab
    lines.push(Line::from(Span::styled("MERGE VIEW TAB", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [↑/k]  ", key_style),
        Span::styled("Previous conflict file", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [↓/j]  ", key_style),
        Span::styled("Next conflict file", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [e]    ", key_style),
        Span::styled("Open in external editor", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [r]    ", key_style),
        Span::styled("Re-scan for conflicts", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Esc]  ", key_style),
        Span::styled("Back to Status", desc_style),
    ]));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled("REMOTE TAB", header_style)));
    lines.push(Line::from(vec![
        Span::styled("  [↑/k]  ", key_style),
        Span::styled("Move up", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [↓/j]  ", key_style),
        Span::styled("Move down", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [a]    ", key_style),
        Span::styled("Add remote (name, then URL)", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [d]    ", key_style),
        Span::styled("Remove selected remote", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [g/G]  ", key_style),
        Span::styled("Top / Bottom", desc_style),
    ]));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "CONFLICT RESOLUTION",
        header_style,
    )));
    lines.push(Line::from(vec![
        Span::styled("  [↑/k]  ", key_style),
        Span::styled("Navigate conflicted files", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [1]    ", key_style),
        Span::styled("Choose 'ours' for selected file", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [2]    ", key_style),
        Span::styled("Choose 'theirs' for selected file", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [m]    ", key_style),
        Span::styled("Mark selected file as resolved", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Esc]  ", key_style),
        Span::styled("Exit conflict resolution", desc_style),
    ]));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "CHECKOUT CONFIRMATION",
        header_style,
    )));
    lines.push(Line::from(vec![
        Span::styled("  [y]        ", key_style),
        Span::styled("Confirm checkout", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [n]/[Esc]  ", key_style),
        Span::styled("Cancel checkout", desc_style),
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
