//! Event handling for the TUI.

use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

/// Poll for the next terminal event with a 250ms timeout.
pub fn poll_event() -> Result<Option<Event>, std::io::Error> {
    if event::poll(Duration::from_millis(250))? {
        match event::read()? {
            CrosstermEvent::Key(key) => Ok(Some(Event::Key(key))),
            CrosstermEvent::Resize(w, h) => Ok(Some(Event::Resize(w, h))),
            _ => Ok(None),
        }
    } else {
        Ok(None)
    }
}

/// TUI events.
#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    Resize(u16, u16),
}

/// Check if a key event matches the given code and modifiers.
#[must_use] 
pub fn key_matches(key: KeyEvent, code: KeyCode, mods: KeyModifiers) -> bool {
    key.code == code && key.modifiers == mods
}
