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

    /// Whether the standard Device Status Reports (`CSI 5 n` and `CSI 6 n`)
    /// are answered by the terminal model instead of by the SSH layer.
    ///
    /// The SSH reader cannot know the terminal state, so it answers both
    /// with fixed legacy replies for a frontend that draws the bytes itself.
    /// A frontend that owns a terminal model needs the queries in its
    /// parser input instead; the model then answers through the same ordered
    /// path as user input. The DEC-private `CSI ? 6 n`, the device attributes
    /// and XTGETTCAP are never part of this switch. The default keeps the
    /// fixed replies for every existing sink.
    ///
    /// Implementations must be constant for the lifetime of the sink: the
    /// reader caches the answer per connection.
    fn forwards_standard_dsr(&self) -> bool {
        false
    }
}
