//! Synchronous session output, lifecycle and flow-control adapter contracts.

use std::time::Duration;

use crate::events::EventSink;
use crate::events::ssh::SshConnectionProgressPayload;

/// Output arriving within this window of successful input may bypass batching.
///
/// After a quiet spell, this keeps an echo or redrawn prompt immediate. Output
/// without recent input still waits one batching window, because programs may
/// draw a frame in pieces and showing the first piece alone causes flicker.
pub const INPUT_ANSWER_WINDOW: Duration = Duration::from_millis(150);

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
