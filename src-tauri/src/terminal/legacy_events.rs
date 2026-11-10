//! Existing terminal event transport, retained until the session sink is wired.

use tauri::{AppHandle, Emitter, Runtime};

// P1-06 captures these contracts before replacing the legacy transport.
pub(crate) const TARGET: tauri::EventTarget = tauri::EventTarget::Any;

pub(crate) fn emit_output<R: Runtime>(app: &AppHandle<R>, tab_id: &str, text: String) {
    let event_name = format!("pty-output-{tab_id}");
    let _ = app.emit_to(TARGET, &event_name, text);
}

pub(crate) fn emit_exit<R: Runtime>(app: &AppHandle<R>, tab_id: &str, reason: Option<&str>) {
    let event_name = format!("pty-exit-{tab_id}");
    let _ = app.emit_to(TARGET, &event_name, reason);
}

pub(crate) fn status_line(color: &str, message: &str) -> String {
    format!("\r\n\x1b[{color}m[{message}]\x1b[0m\r\n")
}

pub(crate) fn jump_status_line(color: &str, hop_index: usize, message: &str) -> String {
    format!("\r\n\x1b[{color}m[Jump #{hop_index}: {message}]\x1b[0m\r\n")
}

#[cfg(test)]
mod tests;
