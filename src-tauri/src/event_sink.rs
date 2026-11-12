//! Tauri's event transport for typed core business events.

use tterm_core::events::EventName;
use tterm_core::events::{AppEvent, EventSink, EventTarget};

mod background; // P1-03.
#[cfg(test)]
mod contracts;
mod launch_dir; // P1-03.
mod notification; // P1-03.
mod proxy; // P1-05.
pub(crate) mod search; // P1-04.
mod serial; // P1-03.
mod session_log; // P1-03.
mod sftp;
mod ssh; // P1-03.
mod terminal; // P1-03.
mod tunnel;
pub(crate) mod zmodem; // P1-03. // P1-04.

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

/// Sends typed core events through Tauri while preserving existing event names,
/// JSON payloads and delivery targets consumed by the WebView.
#[derive(Clone)]
pub(crate) struct TauriEventSink<R: tauri::Runtime = tauri::Wry> {
    app: AppHandle<R>,
}

impl<R: tauri::Runtime> TauriEventSink<R> {
    pub(crate) fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl TauriEventSink {
    /// Maps domain payloads without touching the window or transport.
    pub(crate) fn wire(event: &AppEvent) -> Result<(EventName, EventTarget, Value), String> {
        let payload = match event {
            AppEvent::Proxy(event) => proxy::wire(event)?, // P1-05.
            AppEvent::Tunnel(event) => tunnel::wire(event)?,
            AppEvent::LaunchDir(event) => launch_dir::wire(event)?, // P1-03.
            AppEvent::Notification(event) => notification::wire(event)?, // P1-03.
            AppEvent::Terminal(event) => terminal::wire(event)?,    // P1-03.
            AppEvent::SessionLog(event) => session_log::wire(event)?, // P1-03.
            AppEvent::Serial(event) => serial::wire(event)?,        // P1-03.
            AppEvent::Zmodem(event) => zmodem::wire(event)?,        // P1-03.
            AppEvent::Background(event) => background::wire(event)?, // P1-03.
            AppEvent::Ssh(event) => ssh::wire(event)?,              // P1-03.
            AppEvent::Sftp(event) => sftp::wire(event)?,            // P1-04.
        };
        Ok((event.name(), event.target(), payload))
    }
}

impl<R: tauri::Runtime> TauriEventSink<R> {
    fn emit_payload<T: Serialize + Clone>(
        &self,
        name: &str,
        target: EventTarget,
        payload: T,
    ) -> Result<(), String> {
        match target {
            EventTarget::Default => self
                .app
                .emit(name, payload)
                .map_err(|error| error.to_string()),
            EventTarget::Any => self
                .app
                .emit_to(tauri::EventTarget::any(), name, payload)
                .map_err(|error| error.to_string()),
        }
    }
}

impl<R: tauri::Runtime> EventSink for TauriEventSink<R> {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        let (name, target, payload) = TauriEventSink::wire(&event)?;
        self.emit_payload(&name.as_string(), target, payload)
    }
}

#[cfg(test)]
mod p1_03_tests;
