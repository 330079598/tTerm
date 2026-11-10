use serde_json::Value;
use tterm_core::events::serial::SerialEvent;

pub(super) fn wire(event: &SerialEvent) -> Result<Value, String> {
    match event {
        SerialEvent::Transfer { event, .. } => return super::zmodem::transfer_wire(event),
    }
    .map_err(|error| error.to_string())
}
