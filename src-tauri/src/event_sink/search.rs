//! The existing search IPC Channel transport (P1-04).
use tauri::ipc::Channel;
use tterm_core::sftp::search::{SearchEvent, SearchResultSink};

/// Forwards search updates directly to the existing IPC Channel without a queue
/// or UI acknowledgement. Search callers retain their send-error policy.
pub(crate) struct TauriSearchResultSink {
    channel: Channel<SearchEvent>,
}
impl TauriSearchResultSink {
    /// Wraps the command's Channel without changing its payload protocol.
    pub(crate) fn new(channel: Channel<SearchEvent>) -> Self {
        Self { channel }
    }
}
impl SearchResultSink for TauriSearchResultSink {
    fn send(&self, event: SearchEvent) -> Result<(), String> {
        self.channel.send(event).map_err(|error| error.to_string())
    }
}
