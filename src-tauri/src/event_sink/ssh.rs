use tterm_core::events::ssh::SshEvent;

use serde_json::Value;

pub(super) fn wire(event: &SshEvent) -> Result<Value, String> {
    match event {
        // P1-05.
        SshEvent::HostKeyPrompt { payload, .. } => serde_json::to_value(payload),
        SshEvent::AuthPrompt { payload, .. } => serde_json::to_value(payload),
        SshEvent::AuthPromptExpired { request_id, .. } => serde_json::to_value(request_id),
        SshEvent::ConnectionProgress { payload, .. } => serde_json::to_value(payload),
        SshEvent::RemoteOs(payload) => serde_json::to_value(payload),
    }
    .map_err(|error| error.to_string())
}
