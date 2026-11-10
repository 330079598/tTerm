use serde_json::Value;
use tterm_core::events::launch_dir::LaunchDirEvent;

pub(super) fn wire(event: &LaunchDirEvent) -> Result<Value, String> {
    match event {
        LaunchDirEvent::Queued => serde_json::to_value(()),
    }
    .map_err(|error| error.to_string())
}
