use super::output_tail::OutputTail;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvError, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};
use tauri::ipc::{Channel, InvokeResponseBody};

/// Bounded queue capacity between the reader and the sender thread. When the
/// webview cannot keep up (see [`OutputFlow`]), the reader blocks here instead
/// of growing memory without limit (the PTY kernel buffer or the SSH window
/// then applies backpressure upstream).
const QUEUE_CAPACITY_MESSAGES: usize = 16;
/// Flush at the latest when this many bytes have accumulated, so a sustained
/// flood cannot keep growing the batch until the interval alone fires.
const FLUSH_THRESHOLD_BYTES: usize = 128 * 1024;
/// Hard cap for a single IPC message; larger batches are split across sends.
const MAX_CHUNK_BYTES: usize = 512 * 1024;
/// Batching window while data keeps arriving (matches a 60-120 FPS cadence).
pub const BATCH_INTERVAL: Duration = Duration::from_millis(8);
/// Bytes sent to the webview but not yet acknowledged as parsed by xterm.js
/// before the sender stops. Large enough to keep the IPC pipe busy, small
/// enough that Ctrl+C reaches the screen within a frame or two.
const HIGH_WATERMARK_BYTES: usize = 1024 * 1024;
/// How long the sender waits without any acknowledgement before assuming the
/// webview lost them (reload, crashed renderer) and sending again anyway.
const ACK_STALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Flow control between the sender thread and xterm.js. `Channel::send` only
/// queues a message for the webview, so without acknowledgements a flood
/// (`cat` on a large file) piles up in the webview faster than xterm can parse
/// it and freezes the UI. The frontend reports parsed bytes via
/// `ack_pty_output`; the sender waits while too many are outstanding.
///
/// One instance lives as long as the frontend's output channel, so automatic
/// SSH reconnect attempts share it and late acknowledgements still count.
pub struct OutputFlow {
    state: Mutex<FlowState>,
    acked: Condvar,
    stall_timeout: Duration,
}

#[derive(Default)]
struct FlowState {
    unacked: usize,
    closed: bool,
}

impl Default for OutputFlow {
    fn default() -> Self {
        Self::with_stall_timeout(ACK_STALL_TIMEOUT)
    }
}

impl OutputFlow {
    fn with_stall_timeout(stall_timeout: Duration) -> Self {
        Self {
            state: Mutex::new(FlowState::default()),
            acked: Condvar::new(),
            stall_timeout,
        }
    }

    /// Records `bytes` as parsed by the frontend.
    pub fn ack(&self, bytes: usize) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.unacked = state.unacked.saturating_sub(bytes);
        self.acked.notify_all();
    }

    /// Forgets the outstanding bytes: a new page acknowledges only what it
    /// receives itself.
    fn reset(&self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .unacked = 0;
        self.acked.notify_all();
    }

    /// Stops waiting for acknowledgements for good, so a session being torn
    /// down can never wedge its sender (and the reader blocked behind it).
    pub fn close(&self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .closed = true;
        self.acked.notify_all();
    }

    /// Blocks while the webview is too far behind, then counts `bytes` as
    /// outstanding.
    fn reserve(&self, bytes: usize) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        while !state.closed && state.unacked >= HIGH_WATERMARK_BYTES {
            let before = state.unacked;
            let (next, wait) = self
                .acked
                .wait_timeout(state, self.stall_timeout)
                .unwrap_or_else(PoisonError::into_inner);
            state = next;
            if wait.timed_out() && state.unacked == before {
                state.unacked = 0;
            }
        }
        state.unacked += bytes;
    }
}

/// The frontend's output channel plus the flow control shared by every
/// sender writing to it. Clones share the channel, so a session moved to a
/// new page (see [`TerminalOutput::move_to`]) keeps every sender writing.
#[derive(Clone)]
pub struct TerminalOutput {
    channel: Arc<Mutex<Channel<InvokeResponseBody>>>,
    flow: Arc<OutputFlow>,
}

impl TerminalOutput {
    pub fn new(channel: Channel<InvokeResponseBody>) -> Self {
        Self::with_flow(channel, Arc::new(OutputFlow::default()))
    }

    fn with_flow(channel: Channel<InvokeResponseBody>, flow: Arc<OutputFlow>) -> Self {
        Self {
            channel: Arc::new(Mutex::new(channel)),
            flow,
        }
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
    fn send(&self, chunk: Vec<u8>) -> tauri::Result<()> {
        self.flow.reserve(chunk.len());
        self.current_channel().send(InvokeResponseBody::Raw(chunk))
    }
}

/// Returns the length of the longest prefix of `data` that does not end in the
/// middle of a UTF-8 sequence, so the remainder can stay buffered until the
/// next read instead of being corrupted into replacement characters.
pub fn utf8_complete_prefix_len(data: &[u8]) -> usize {
    if data.is_empty() {
        return 0;
    }
    // A well-formed sequence is at most 4 bytes, so only the tail can be partial.
    let max_skip = 4.min(data.len());
    for skip in 1..=max_skip {
        let start = data.len() - skip;
        let Some(expected) = utf8_lead_len(data[start]) else {
            continue;
        };
        // A trailing 1-byte sequence is always complete; only a multi-byte
        // lead with fewer bytes present than expected is a partial tail.
        let available = data.len() - start;
        if expected > 1
            && expected > available
            && (1..available).all(|offset| data[start + offset] & 0xc0 == 0x80)
        {
            return start;
        }
    }
    data.len()
}

/// Length of the UTF-8 sequence introduced by `byte`, or `None` for
/// continuation bytes and invalid leads.
fn utf8_lead_len(byte: u8) -> Option<usize> {
    match byte {
        0x00..=0x7f => Some(1),
        0xc0..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf7 => Some(4),
        _ => None,
    }
}

/// A `Send` handle for hot-path terminal output. Data flows through a bounded
/// queue to a dedicated sender thread, which flushes to the frontend channel
/// in UTF-8-safe, size-capped batches.
pub struct TerminalOutputSender {
    tx: Option<SyncSender<Vec<u8>>>,
    worker: Option<std::thread::JoinHandle<()>>,
    closed: bool,
    tail: Option<Arc<OutputTail>>,
    output_counter: Option<Arc<AtomicU64>>,
}

impl TerminalOutputSender {
    pub fn spawn(tab_id: &str, output: TerminalOutput) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(QUEUE_CAPACITY_MESSAGES);
        let worker_tab_id = tab_id.to_string();
        let worker = std::thread::Builder::new()
            .name(format!("pty-out-{worker_tab_id}"))
            .spawn(move || sender_loop(&rx, &output))
            .expect("spawn pty output sender thread");
        Self {
            tx: Some(tx),
            worker: Some(worker),
            closed: false,
            tail: None,
            output_counter: None,
        }
    }

    /// Also records every sent byte into `tail`.
    pub fn with_output_tail(mut self, tail: Arc<OutputTail>) -> Self {
        self.tail = Some(tail);
        self
    }

    /// Enqueues raw bytes; blocks once the bounded queue is full. No-op after
    /// the worker has stopped so a dying webview cannot wedge the reader.
    /// Counts every byte sent, which paces pastes (see `PtyInput`).
    pub fn with_output_counter(mut self, counter: Arc<AtomicU64>) -> Self {
        self.output_counter = Some(counter);
        self
    }

    pub fn send(&self, data: Vec<u8>) {
        if data.is_empty() || self.closed {
            return;
        }
        if let Some(counter) = &self.output_counter {
            counter.fetch_add(data.len() as u64, Ordering::SeqCst);
        }
        if let Some(tail) = &self.tail {
            tail.record(&data);
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(data);
        }
    }

    /// Signals end-of-stream and waits until every queued byte has been
    /// delivered to the webview. Call before emitting the session exit event
    /// so trailing output is never dropped.
    pub fn finish(&mut self) {
        self.closed = true;
        // Dropping the only sender wakes the worker's recv with Disconnected;
        // it flushes any remaining batch and exits. The worker may still be
        // draining a full queue, so join after the drop.
        if let (Some(tx), Some(worker)) = (self.tx.take(), self.worker.take()) {
            drop(tx);
            let _ = worker.join();
        }
    }
}

impl Drop for TerminalOutputSender {
    fn drop(&mut self) {
        self.finish();
    }
}

fn sender_loop(rx: &Receiver<Vec<u8>>, channel: &TerminalOutput) {
    let mut pending: Vec<u8> = Vec::with_capacity(FLUSH_THRESHOLD_BYTES);
    // Deadline of the open batching window: set when the first unwritten
    // byte arrives, never extended by later arrivals. A self-resetting
    // recv_timeout would starve a steady trickle (messages every few ms,
    // below the byte threshold) until megabytes accumulated.
    let mut flush_deadline: Option<Instant> = None;
    let mut alive = true;
    while alive {
        let message = match flush_deadline {
            Some(deadline) => {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(data) => Some(data),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => {
                        alive = false;
                        None
                    }
                }
            }
            // No batching window is open, so no timer can expire usefully:
            // block until the next byte arrives. finish() wakes this via
            // Disconnected when the sender side is dropped. Deferred partial
            // UTF-8 bytes (if any) need more input, not a timeout, to flush.
            None => match rx.recv() {
                Ok(data) => Some(data),
                Err(RecvError) => {
                    alive = false;
                    None
                }
            },
        };
        if let Some(data) = message {
            if flush_deadline.is_none() {
                flush_deadline = Some(Instant::now() + BATCH_INTERVAL);
            }
            pending.extend_from_slice(&data);
            if pending.len() >= FLUSH_THRESHOLD_BYTES {
                flush_all(&mut pending, channel);
                flush_deadline = None;
                // Bytes queued behind this batch are already deliverable:
                // drain them now instead of waking once per message.
                loop {
                    match rx.try_recv() {
                        Ok(rest) => {
                            pending.extend_from_slice(&rest);
                            if pending.len() >= MAX_CHUNK_BYTES {
                                flush_all(&mut pending, channel);
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            alive = false;
                            break;
                        }
                    }
                }
                if alive && !pending.is_empty() {
                    flush_deadline = Some(Instant::now() + BATCH_INTERVAL);
                }
            }
        } else if flush_deadline.take().is_some() {
            // Batching window elapsed: deliver what accumulated.
            flush_all(&mut pending, channel);
        }
    }
    // End of stream: deliver any deferred partial sequence as-is.
    flush_final(&mut pending, channel);
}

/// Flushes the buffer in UTF-8-safe, size-capped chunks. Any trailing partial
/// sequence stays buffered for the next flush.
fn flush_all(pending: &mut Vec<u8>, channel: &TerminalOutput) {
    if pending.is_empty() {
        return;
    }
    let mut start = 0;
    while start < pending.len() {
        let limit = (start + MAX_CHUNK_BYTES).min(pending.len());
        let end = start + utf8_complete_prefix_len(&pending[start..limit]);
        if end == start {
            // The whole window is one partial sequence; wait for more bytes.
            break;
        }
        let chunk: Vec<u8> = pending[start..end].to_vec();
        if channel.send(chunk).is_err() {
            // The webview is gone; nothing useful left to do.
            pending.clear();
            return;
        }
        start = end;
    }
    pending.drain(..start);
    if pending.capacity() > FLUSH_THRESHOLD_BYTES * 4 && pending.len() < FLUSH_THRESHOLD_BYTES {
        pending.shrink_to_fit();
    }
}

/// End-of-stream variant: no more bytes will arrive, so send everything that
/// is left (a trailing partial sequence goes out as-is) instead of buffering.
fn flush_final(pending: &mut Vec<u8>, channel: &TerminalOutput) {
    while !pending.is_empty() {
        let limit = MAX_CHUNK_BYTES.min(pending.len());
        let chunk: Vec<u8> = pending.drain(..limit).collect();
        if channel.send(chunk).is_err() {
            pending.clear();
            return;
        }
    }
}

#[cfg(test)]
mod tests;
