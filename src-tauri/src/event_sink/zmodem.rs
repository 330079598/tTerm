use serde_json::Value;
use tterm_core::events::zmodem::ZmodemEvent;

pub(super) fn wire(event: &ZmodemEvent) -> Result<Value, String> {
    match event {
        ZmodemEvent::SendRequested { .. } => serde_json::to_value(()),
        ZmodemEvent::Transfer { event, .. } => return transfer_wire(event),
    }
    .map_err(|error| error.to_string())
}
pub(crate) fn transfer_wire(
    event: &tterm_core::events::zmodem::TransferEvent,
) -> Result<Value, String> {
    use tterm_core::events::zmodem::TransferEvent;
    match event {
        TransferEvent::Start(p) => serde_json::to_value(p),
        TransferEvent::Progress(p) => serde_json::to_value(p),
        TransferEvent::Complete(p) => serde_json::to_value(p),
    }
    .map_err(|error| error.to_string())
}
