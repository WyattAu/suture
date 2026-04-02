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
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
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
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new(repo);
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
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
                Event::Resize => {
                    terminal
                        .resize(ratatui::layout::Rect::new(0, 0, 80, 24))
                        .ok();
                }
            }
        }
    }
}
