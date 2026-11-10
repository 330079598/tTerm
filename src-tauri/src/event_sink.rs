//! Tauri's event transport for typed core business events.

mod background; // P1-03.
mod ssh; // P1-03.
mod tunnel;
pub(crate) mod zmodem; // P1-03.

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tterm_core::events::EventName;
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

    /// Maps domain payloads without touching the window or transport.
    pub(crate) fn wire(event: &AppEvent) -> Result<(EventName, EventTarget, Value), String> {
        let payload = match event {
            AppEvent::Tunnel(event) => tunnel::wire(event)?,
            AppEvent::Zmodem(event) => zmodem::wire(event)?, // P1-03.
            AppEvent::Background(event) => background::wire(event)?, // P1-03.
            AppEvent::Ssh(event) => ssh::wire(event)?,       // P1-03.
        };
        Ok((event.name(), event.target(), payload))
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

#[cfg(test)]
mod legacy_contract_tests;
