use serde_json::Value;
use tterm_core::events::proxy::ProxyEvent;

/// P1-05: serializes only the legacy payload, without an enum envelope.
pub(super) fn wire(event: &ProxyEvent) -> Result<Value, String> {
    match event {
        ProxyEvent::CommandPrompt(payload) => serde_json::to_value(payload),
        ProxyEvent::CommandPromptExpired(id) => serde_json::to_value(id),
    }
    .map_err(|error| error.to_string())
}
