use tterm_core::events::serial::SerialEvent;

use serde_json::Value;

pub(super) fn wire(event: &SerialEvent) -> Result<Value, String> {
    match event {
        SerialEvent::Transfer { event, .. } => tterm_core::zmodem::wire::transfer_wire(event),
    }
}
