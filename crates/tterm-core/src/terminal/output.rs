//! Byte output endpoint shared by session senders.
use crate::session::sink::INPUT_ANSWER_WINDOW;
use crate::terminal::io_batcher::OutputFlow;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

type SendBytes = Arc<dyn Fn(Vec<u8>) -> Result<(), String> + Send + Sync>;

/// The output endpoint plus the flow control shared by every
/// sender writing to it. Clones share the channel, so a session moved to a
/// new page (see [`TerminalOutput::move_to`]) keeps every sender writing.
#[derive(Clone)]
pub struct TerminalOutput {
    channel: Arc<Mutex<SendBytes>>,
    flow: Arc<OutputFlow>,
    last_input: Arc<Mutex<Option<Instant>>>,
}

impl TerminalOutput {
    /// Couples a byte sender with its existing flow control.
    pub fn new(send: SendBytes, flow: Arc<OutputFlow>) -> Self {
        Self {
            channel: Arc::new(Mutex::new(send)),
            flow,
            last_input: Arc::new(Mutex::new(None)),
        }
    }

    /// Records that input went to the session, so the output answering it
    /// goes out without waiting for a batching window.
    pub fn note_input(&self) {
        *self
            .last_input
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }

    /// Whether input went to the session within [`INPUT_ANSWER_WINDOW`].
    pub fn answers_input(&self) -> bool {
        self.last_input
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some_and(|at| at.elapsed() < INPUT_ANSWER_WINDOW)
    }

    /// Sends from now on to `other`'s channel, the one a reloaded page
    /// listens on; the old page and its acknowledgements are gone.
    pub fn move_to(&self, other: &TerminalOutput) {
        let channel = other.current_channel();
        *self.channel.lock().unwrap_or_else(PoisonError::into_inner) = channel;
        self.flow.reset();
    }

    fn current_channel(&self) -> SendBytes {
        self.channel
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn flow(&self) -> &Arc<OutputFlow> {
        &self.flow
    }

    /// Sends one chunk once the webview has room for it.
    pub fn send(&self, chunk: Vec<u8>) -> Result<(), String> {
        self.flow.reserve(chunk.len());
        self.current_channel()(chunk)
    }
}
