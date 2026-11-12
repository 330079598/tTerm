//! P1-06: cold terminal reports through the typed event adapter.

use crate::events::terminal::TerminalEvent;
use crate::events::{AppEvent, EventSink};

pub fn emit_output(events: &dyn EventSink, tab_id: &str, text: String) {
    let _ = events.emit(AppEvent::Terminal(TerminalEvent::Output {
        tab_id: tab_id.to_string(),
        text,
    }));
}

pub fn status_line(color: &str, message: &str) -> String {
    format!("\r\n\x1b[{color}m[{message}]\x1b[0m\r\n")
}

pub fn jump_status_line(color: &str, hop_index: usize, message: &str) -> String {
    format!("\r\n\x1b[{color}m[Jump #{hop_index}: {message}]\x1b[0m\r\n")
}

#[cfg(test)]
mod tests;
