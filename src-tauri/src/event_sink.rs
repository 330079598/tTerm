//! Tauri's event transport for typed core business events.

// P1-04 owns the SFTP notification and search transports.
pub(crate) mod search;
mod sftp;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tterm_core::events::tunnel::TunnelEvent;
use tterm_core::events::{AppEvent, EventName, EventSink, EventTarget};

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

    /// Returns the wire representation used by Tauri. Keeping this mapping
    /// separate makes the compatibility contract testable without a window.
    pub(crate) fn wire(event: &AppEvent) -> Result<(EventName, EventTarget, Value), String> {
        match event {
            // P1-04 owns SFTP serialization; routing comes from the core event.
            AppEvent::Sftp(payload) => Ok((event.name(), event.target(), sftp::wire(payload)?)),
            AppEvent::Tunnel(TunnelEvent::Status(status)) => Ok((
                event.name(),
                event.target(),
                serde_json::to_value(status).map_err(|error| error.to_string())?,
            )),
        }
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
mod tests {
    use super::*;
    use tterm_core::events::tunnel::{TunnelFailure, TunnelState, TunnelStatus, TunnelTraffic};

    #[test]
    fn tunnel_status_wire_contract_matches_existing_tauri_event() {
        let status = TunnelStatus {
            id: "t1".to_string(),
            state: TunnelState::Running,
            message: None,
            bound_port: Some(15432),
            active_connections: 1,
            total_connections: 2,
            failed_connections: 3,
            bytes_up: 4,
            bytes_down: 5,
            connected_at: Some(1_700_000_000_000),
            retry_attempt: 0,
            last_failure: Some(TunnelFailure {
                message: "refused".to_string(),
                at: 1_700_000_000_001,
            }),
            lifetime: TunnelTraffic {
                bytes_up: 6,
                bytes_down: 7,
                connections: 8,
                since: Some(1_700_000_000_002),
            },
        };
        let event = AppEvent::Tunnel(TunnelEvent::Status(status));
        let (name, target, payload) = TauriEventSink::wire(&event).expect("wire event");
        assert_eq!(name.as_string(), "tunnel-status");
        assert_eq!(target, EventTarget::Default);
        assert_eq!(
            payload,
            serde_json::json!({
                "id": "t1",
                "state": "running",
                "message": null,
                "boundPort": 15432,
                "activeConnections": 1,
                "totalConnections": 2,
                "failedConnections": 3,
                "bytesUp": 4,
                "bytesDown": 5,
                "connectedAt": 1700000000000u64,
                "retryAttempt": 0,
                "lastFailure": {"message": "refused", "at": 1700000000001u64},
                "lifetime": {"bytesUp": 6, "bytesDown": 7, "connections": 8, "since": 1700000000002u64}
            })
        );
    }
}
