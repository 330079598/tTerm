use super::{AppEvent, EventSink};
use std::sync::{Arc, Mutex};

/// An in-memory event sink used by core tests and adapter tests.
#[derive(Clone, Default)]
pub struct RecordingEventSink {
    events: Arc<Mutex<Vec<AppEvent>>>,
}

impl RecordingEventSink {
    /// Returns a snapshot of all events received so far.
    pub fn events(&self) -> Vec<AppEvent> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl EventSink for RecordingEventSink {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(event);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventName;
    use crate::events::tunnel::{TunnelEvent, TunnelStatus, TunnelTraffic};

    #[test]
    fn records_typed_events_without_changing_payloads() {
        let sink = RecordingEventSink::default();
        let status = TunnelStatus::stopped("t1", TunnelTraffic::default());
        sink.emit(AppEvent::Tunnel(TunnelEvent::Status(status.clone())))
            .expect("recording sink cannot fail");

        assert_eq!(
            sink.events(),
            vec![AppEvent::Tunnel(TunnelEvent::Status(status))]
        );
    }

    #[test]
    fn tab_scoped_names_keep_the_existing_suffix_shape() {
        let name = EventName::TabScoped {
            prefix: "sftp-upload-batch-complete",
            tab_id: "tab-7".to_string(),
        };
        assert_eq!(name.as_string(), "sftp-upload-batch-complete-tab-7");
    }
}
