//! P1-06: cold terminal reports through the typed event adapter.
use tauri::{AppHandle, Runtime};
use tterm_core::events::terminal::TerminalEvent;
use tterm_core::events::{AppEvent, EventSink};

pub(crate) fn emit_output<R: Runtime>(app: &AppHandle<R>, tab_id: &str, text: String) {
    let _ = crate::event_sink::TauriEventSink::new(app.clone()).emit(AppEvent::Terminal(
        TerminalEvent::Output {
            tab_id: tab_id.to_string(),
            text,
        },
    ));
}

pub(crate) fn status_line(color: &str, message: &str) -> String {
    format!("\r\n\x1b[{color}m[{message}]\x1b[0m\r\n")
}

pub(crate) fn jump_status_line(color: &str, hop_index: usize, message: &str) -> String {
    format!("\r\n\x1b[{color}m[Jump #{hop_index}: {message}]\x1b[0m\r\n")
}

#[cfg(test)]
mod tests;
