use tterm_core::events::serial::SerialEvent;

use serde_json::Value;

pub(super) fn wire(event: &SerialEvent) -> Result<Value, String> {
    match event {
        SerialEvent::Transfer { event, .. } => super::zmodem::transfer_wire(event),
    }
}
