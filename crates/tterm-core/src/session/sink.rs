use crate::events::EventSink;

/// Receives a tab's already batched output on its existing sender thread.
///
/// Methods are synchronous. Implementations must not acquire the session map,
/// active-session or protocol locks: a caller may already own those locks.
/// P1-06 adds connection progress after P1-03 supplies its existing payload.
pub trait SessionSink: EventSink {
    /// Sends raw bytes, waiting for parsed-output acknowledgements if needed.
    fn send_bytes(&self, bytes: Vec<u8>) -> Result<(), String>;

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
