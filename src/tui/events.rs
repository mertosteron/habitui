use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind};

const POLL_INTERVAL_MS: u64 = 250;

/// Poll for a key press event. Returns `Ok(None)` on timeout, on non-key events,
/// or on key release/repeat (we only surface `KeyEventKind::Press`).
pub fn next_key() -> io::Result<Option<KeyEvent>> {
    if !event::poll(Duration::from_millis(POLL_INTERVAL_MS))? {
        return Ok(None);
    }
    match event::read()? {
        Event::Key(k) if k.kind == KeyEventKind::Press => Ok(Some(k)),
        _ => Ok(None),
    }
}
