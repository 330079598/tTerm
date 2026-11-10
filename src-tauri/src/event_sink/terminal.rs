use serde_json::Value;
use tterm_core::events::terminal::TerminalEvent;

pub(super) fn wire(event: &TerminalEvent) -> Result<Value, String> {
    match event {
        TerminalEvent::Cwd { payload, .. } => serde_json::to_value(payload),
    }
    .map_err(|error| error.to_string())
}
