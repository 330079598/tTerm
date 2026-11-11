//! Binary IPC output adapter, separate from session batching.
use crate::terminal::io_batcher::OutputFlow;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;
use tauri::ipc::{Channel, InvokeResponseBody};
use tterm_core::session::sink::INPUT_ANSWER_WINDOW;

/// The frontend's output channel plus the flow control shared by every
/// sender writing to it. Clones share the channel, so a session moved to a
/// new page (see [`TerminalOutput::move_to`]) keeps every sender writing.
#[derive(Clone)]
pub struct TerminalOutput {
    channel: Arc<Mutex<Channel<InvokeResponseBody>>>,
    flow: Arc<OutputFlow>,
    last_input: Arc<Mutex<Option<Instant>>>,
}

impl TerminalOutput {
    pub fn new(channel: Channel<InvokeResponseBody>) -> Self {
        Self::with_flow(channel, Arc::new(OutputFlow::default()))
    }

    pub(crate) fn with_flow(channel: Channel<InvokeResponseBody>, flow: Arc<OutputFlow>) -> Self {
        Self {
            channel: Arc::new(Mutex::new(channel)),
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
    pub(crate) fn answers_input(&self) -> bool {
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

    fn current_channel(&self) -> Channel<InvokeResponseBody> {
        self.channel
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn flow(&self) -> &Arc<OutputFlow> {
        &self.flow
    }

    /// Sends one chunk once the webview has room for it.
    pub(crate) fn send(&self, chunk: Vec<u8>) -> tauri::Result<()> {
        self.flow.reserve(chunk.len());
        self.current_channel().send(InvokeResponseBody::Raw(chunk))
    }
}
