//! Suture TUI — Terminal User Interface for Suture USVCS.
//!
//! Provides an interactive terminal UI built on `ratatui` and `crossterm`
//! with views for status, log graph, interactive staging, diff viewing,
//! and merge conflict resolution.

pub mod app;
pub mod event;
pub mod ui;

use std::io;

use app::App;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    cursor::Show,
};
use event::Event;
use ratatui::Terminal;
use ratatui::prelude::CrosstermBackend;
use suture_core::repository::Repository;

/// Run the Suture TUI for the repository at the given path.
pub fn run(repo_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    // Open the repository
    let repo =
        Repository::open(repo_path).map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new(repo);
    let result = run_app(&mut terminal, &mut app);

    // Install panic hook to restore terminal on crash
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, Show);
        original_hook(panic_info);
    }));

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    app.refresh()?;

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if let Some(event) = event::poll_event()? {
            match event {
                Event::Key(key) => {
                    if app.handle_key(key) {
                        // App requested quit
                        return Ok(());
                    }
                }
                Event::Resize(w, h) => {
                    terminal
                        .resize(ratatui::layout::Rect::new(0, 0, w, h))
                        .ok();
                }
            }
        }
    }
}
