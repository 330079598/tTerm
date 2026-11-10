//! Tauri's event transport for typed core business events.

mod tunnel;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tterm_core::events::{AppEvent, EventSink, EventTarget};

/// Sends typed core events through Tauri while preserving existing event names,
/// JSON payloads and delivery targets consumed by the WebView.
#[derive(Clone)]
pub(crate) struct TauriEventSink {
    app: AppHandle,
}

impl TauriEventSink {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }

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

impl EventSink for TauriEventSink {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        let (name, target, payload) = Self::wire(&event)?;
        self.emit_payload(&name.as_string(), target, payload)
    }
}
