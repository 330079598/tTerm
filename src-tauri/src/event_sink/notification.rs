use serde_json::Value;
use tterm_core::events::notification::NotificationEvent;

pub(super) fn wire(event: &NotificationEvent) -> Result<Value, String> {
    match event {
        NotificationEvent::Activated(payload) => serde_json::to_value(payload),
    }
    .map_err(|error| error.to_string())
}
