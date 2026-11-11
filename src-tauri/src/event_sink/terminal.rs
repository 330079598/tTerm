use tterm_core::events::terminal::TerminalEvent;

use serde_json::Value;

pub(super) fn wire(event: &TerminalEvent) -> Result<Value, String> {
    match event {
        TerminalEvent::Output { text, .. } => serde_json::to_value(text), // P1-06.
        TerminalEvent::Exit { reason, .. } => serde_json::to_value(reason), // P1-06.
        TerminalEvent::Cwd { payload, .. } => serde_json::to_value(payload),
    }
    .map_err(|error| error.to_string())
}
