use serde_json::Value;
use tterm_core::events::zmodem::TransferEvent;

pub(crate) fn transfer_wire(event: &TransferEvent) -> Result<Value, String> {
    match event {
        TransferEvent::Start(payload) => serde_json::to_value(payload),
        TransferEvent::Progress(payload) => serde_json::to_value(payload),
        TransferEvent::Complete(payload) => serde_json::to_value(payload),
    }
    .map_err(|error| error.to_string())
}
