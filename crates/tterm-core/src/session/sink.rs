use crate::events::EventSink;
use crate::events::ssh::SshConnectionProgressPayload;

/// Receives a tab's already batched output on its existing sender thread.
///
/// Methods are synchronous. Implementations must not acquire the session map,
/// active-session or protocol locks: a caller may already own those locks.
/// Connection progress uses the existing P1-03 payload without a new state machine.
pub trait SessionSink: EventSink {
    /// Sends raw bytes, waiting for parsed-output acknowledgements if needed.
    fn send_bytes(&self, bytes: Vec<u8>) -> Result<(), String>;

    /// Reports the existing connection phase without adding transitions.
    fn state_changed(&self, progress: SshConnectionProgressPayload) -> Result<(), String>;

    /// Reports the existing optional final reason, without closing output flow.
    fn exited(&self, reason: Option<String>) -> Result<(), String>;

    /// Acknowledges parsed binary bytes after the caller validates the nonce.
    fn ack_output(&self, bytes: usize);

    /// Releases flow-control waiters during permanent teardown, without joining.
    fn close_output(&self);

    /// Records successful input for the existing immediate-echo fast path.
    fn note_input(&self);

    /// Checks whether output is within the existing input-answer window.
    fn answers_input(&self) -> bool;
}
