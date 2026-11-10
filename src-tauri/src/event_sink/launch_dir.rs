use tterm_core::events::launch_dir::LaunchDirEvent;

use serde_json::Value;

pub(super) fn wire(event: &LaunchDirEvent) -> Result<Value, String> {
    match event {
        LaunchDirEvent::Queued => serde_json::to_value(()),
    }
    .map_err(|error| error.to_string())
}
