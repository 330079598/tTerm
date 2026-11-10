use serde_json::Value;
use tterm_core::events::session_log::SessionLogEvent;

pub(super) fn wire(event: &SessionLogEvent) -> Result<Value, String> {
    match event {
        SessionLogEvent::Error(payload) => serde_json::to_value(payload),
        SessionLogEvent::Status(payload) => serde_json::to_value(payload),
    }
    .map_err(|error| error.to_string())
}
