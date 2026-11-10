use tterm_core::events::ssh::SshEvent;

use serde_json::Value;

pub(super) fn wire(event: &SshEvent) -> Result<Value, String> {
    match event {
        SshEvent::ConnectionProgress { payload, .. } => serde_json::to_value(payload),
        SshEvent::RemoteOs(payload) => serde_json::to_value(payload),
    }
    .map_err(|error| error.to_string())
}
