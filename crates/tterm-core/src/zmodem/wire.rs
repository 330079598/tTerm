use crate::events::zmodem::TransferEvent;
use serde_json::Value;

/// Serializes a transfer event into the JSON payload used by the Tauri
/// ZMODEM and serial transfer events.
pub fn transfer_wire(event: &TransferEvent) -> Result<Value, String> {
    match event {
        TransferEvent::Start(payload) => serde_json::to_value(payload),
        TransferEvent::Progress(payload) => serde_json::to_value(payload),
        TransferEvent::Complete(payload) => serde_json::to_value(payload),
    }
    .map_err(|error| error.to_string())
}
