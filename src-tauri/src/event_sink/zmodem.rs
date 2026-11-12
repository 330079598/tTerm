use tterm_core::events::zmodem::ZmodemEvent;

use serde_json::Value;

pub(super) fn wire(event: &ZmodemEvent) -> Result<Value, String> {
    match event {
        ZmodemEvent::SendRequested { .. } => serde_json::to_value(()),
        ZmodemEvent::Transfer { event, .. } => {
            return crate::zmodem::wire::transfer_wire(event);
        }
    }
    .map_err(|error| error.to_string())
}
