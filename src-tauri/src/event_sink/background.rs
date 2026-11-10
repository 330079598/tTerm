use tterm_core::events::background::BackgroundEvent;

use serde_json::Value;

pub(super) fn wire(event: &BackgroundEvent) -> Result<Value, String> {
    match event {
        BackgroundEvent::QuitRequested(payload) => serde_json::to_value(payload),
        _ => serde_json::to_value(()),
    }
    .map_err(|error| error.to_string())
}
