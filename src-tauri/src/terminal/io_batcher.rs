use super::output_tail::OutputTail;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvError, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
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
        let mut state = self.state.lock().unwrap();
        state.unacked = state.unacked.saturating_sub(bytes);
        self.acked.notify_all();
    }

    /// Stops waiting for acknowledgements for good, so a session being torn
    /// down can never wedge its sender (and the reader blocked behind it).
    pub fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.acked.notify_all();
    }

    /// Blocks while the webview is too far behind, then counts `bytes` as
    /// outstanding.
    fn reserve(&self, bytes: usize) {
        let mut state = self.state.lock().unwrap();
        while !state.closed && state.unacked >= HIGH_WATERMARK_BYTES {
            let before = state.unacked;
            let (next, wait) = self.acked.wait_timeout(state, self.stall_timeout).unwrap();
            state = next;
            if wait.timed_out() && state.unacked == before {
                state.unacked = 0;
            }
        }
        state.unacked += bytes;
    }
}

/// The frontend's output channel plus the flow control shared by every
/// sender writing to it.
#[derive(Clone)]
pub struct TerminalOutput {
    channel: Channel<InvokeResponseBody>,
    flow: Arc<OutputFlow>,
}

impl TerminalOutput {
    pub fn new(channel: Channel<InvokeResponseBody>) -> Self {
        Self::with_flow(channel, Arc::new(OutputFlow::default()))
    }

    fn with_flow(channel: Channel<InvokeResponseBody>, flow: Arc<OutputFlow>) -> Self {
        Self { channel, flow }
    }

    pub fn flow(&self) -> &Arc<OutputFlow> {
        &self.flow
    }

    /// Sends one chunk once the webview has room for it.
    fn send(&self, chunk: Vec<u8>) -> tauri::Result<()> {
        self.flow.reserve(chunk.len());
        self.channel.send(InvokeResponseBody::Raw(chunk))
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
mod tests {
    use super::*;

    #[test]
    fn utf8_prefix_keeps_complete_sequences() {
        assert_eq!(utf8_complete_prefix_len(b"abc"), 3);
        assert_eq!(utf8_complete_prefix_len("中文".as_bytes()), 6);
    }

    #[test]
    fn utf8_prefix_defers_partial_trailing_sequence() {
        // "中" is E4 B8 AD; feeding only its first two bytes must hold both.
        let partial = &"中".as_bytes()[..2];
        assert_eq!(utf8_complete_prefix_len(partial), 0);
        // A complete char followed by a partial one keeps only the complete part.
        let mixed = [b'a', 0xe4, 0xb8];
        assert_eq!(utf8_complete_prefix_len(&mixed), 1);
    }

    #[test]
    fn utf8_prefix_handles_four_byte_sequences() {
        let emoji = "😀".as_bytes(); // F0 9F 98 80
        assert_eq!(utf8_complete_prefix_len(&emoji[..3]), 0);
        assert_eq!(utf8_complete_prefix_len(emoji), 4);
    }

    #[test]
    fn utf8_prefix_is_fine_for_plain_ascii_and_controls() {
        let data = b"hello\r\n\x1b[31mworld\x1b[0m";
        assert_eq!(utf8_complete_prefix_len(data), data.len());
    }

    #[test]
    fn utf8_prefix_never_wedges_on_malformed_bytes() {
        // Stray continuation bytes must flush instead of buffering forever.
        assert_eq!(utf8_complete_prefix_len(&[0x80, 0x80, 0x80]), 3);
        // Truncated 4-byte lead followed by garbage also flushes.
        assert_eq!(utf8_complete_prefix_len(&[0xf0, 0x9f, 0x41]), 3);
    }

    struct ChunkRecorder {
        chunks: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    }

    impl ChunkRecorder {
        /// Acknowledges every chunk as soon as it arrives, like a webview
        /// that parses instantly.
        fn channel(&self) -> TerminalOutput {
            let chunks = self.chunks.clone();
            let flow = Arc::new(OutputFlow::default());
            let ack = flow.clone();
            let channel = Channel::new(move |body: InvokeResponseBody| {
                if let InvokeResponseBody::Raw(bytes) = body {
                    ack.ack(bytes.len());
                    chunks.lock().unwrap().push(bytes);
                }
                Ok(())
            });
            TerminalOutput::with_flow(channel, flow)
        }
    }

    /// Records chunks without acknowledging them, like a webview that has
    /// fallen behind.
    fn unacked_output(stall_timeout: Duration) -> (TerminalOutput, Arc<Mutex<Vec<Vec<u8>>>>) {
        let chunks: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = chunks.clone();
        let channel = Channel::new(move |body: InvokeResponseBody| {
            if let InvokeResponseBody::Raw(bytes) = body {
                sink.lock().unwrap().push(bytes);
            }
            Ok(())
        });
        let flow = Arc::new(OutputFlow::with_stall_timeout(stall_timeout));
        (TerminalOutput::with_flow(channel, flow), chunks)
    }

    fn delivered(chunks: &Mutex<Vec<Vec<u8>>>) -> usize {
        chunks.lock().unwrap().iter().map(Vec::len).sum()
    }

    fn feed(mut sender: TerminalOutputSender, total: usize) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            for _ in 0..total / 4096 {
                sender.send(vec![b'x'; 4096]);
            }
            sender.finish();
        })
    }

    #[test]
    fn sender_pauses_at_high_watermark_until_acknowledged() {
        let (output, chunks) = unacked_output(Duration::from_secs(60));
        let flow = output.flow().clone();
        let total = HIGH_WATERMARK_BYTES * 3;
        let feeder = feed(TerminalOutputSender::spawn("flow", output), total);

        std::thread::sleep(Duration::from_millis(200));
        let paused_at = delivered(&chunks);
        assert!(
            paused_at >= HIGH_WATERMARK_BYTES,
            "paused early at {paused_at}"
        );
        assert!(
            paused_at < HIGH_WATERMARK_BYTES + MAX_CHUNK_BYTES,
            "ran past the watermark"
        );

        // Acknowledge everything delivered so far, as the webview would.
        let mut acked = 0;
        while !feeder.is_finished() {
            let now = delivered(&chunks);
            flow.ack(now - acked);
            acked = now;
            std::thread::sleep(Duration::from_millis(5));
        }
        feeder.join().unwrap();
        assert_eq!(delivered(&chunks), total);
    }

    #[test]
    fn close_releases_a_paused_sender() {
        let (output, chunks) = unacked_output(Duration::from_secs(60));
        let flow = output.flow().clone();
        let total = HIGH_WATERMARK_BYTES * 2;
        let feeder = feed(TerminalOutputSender::spawn("flow-close", output), total);

        std::thread::sleep(Duration::from_millis(100));
        assert!(delivered(&chunks) < total);
        flow.close();
        feeder.join().unwrap();
        assert_eq!(delivered(&chunks), total);
    }

    #[test]
    fn stalled_acknowledgements_do_not_wedge_the_sender() {
        let (output, chunks) = unacked_output(Duration::from_millis(50));
        let total = HIGH_WATERMARK_BYTES * 2;
        feed(TerminalOutputSender::spawn("flow-stall", output), total)
            .join()
            .unwrap();
        assert_eq!(delivered(&chunks), total);
    }

    #[test]
    fn flush_all_splits_large_batches_into_capped_chunks() {
        let recorder = ChunkRecorder {
            chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let channel = recorder.channel();
        let mut pending = vec![b'x'; MAX_CHUNK_BYTES + 10];
        flush_all(&mut pending, &channel);

        let chunks = recorder.chunks.lock().unwrap();
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().all(|chunk| chunk.len() <= MAX_CHUNK_BYTES));
        assert!(pending.is_empty());
    }

    #[test]
    fn flush_all_never_emits_partial_utf8_across_chunks() {
        let recorder = ChunkRecorder {
            chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let channel = recorder.channel();
        // ~600k bytes of "中": crosses the MAX_CHUNK boundary mid-character.
        let mut pending = Vec::new();
        for _ in 0..200_003 {
            pending.extend_from_slice("中".as_bytes());
        }
        flush_all(&mut pending, &channel);

        let chunks = recorder.chunks.lock().unwrap();
        assert!(chunks.len() >= 2);
        for chunk in chunks.iter() {
            assert!(std::str::from_utf8(chunk).is_ok(), "chunk split mid-UTF-8");
        }
        assert!(pending.is_empty());
    }

    #[test]
    fn flush_all_buffers_partial_trailing_sequence() {
        let recorder = ChunkRecorder {
            chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let channel = recorder.channel();
        let mut pending = vec![b'a', b'b', 0xe4, 0xb8];
        flush_all(&mut pending, &channel);

        assert_eq!(pending, vec![0xe4, 0xb8]);
        let chunks = recorder.chunks.lock().unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], b"ab");
    }

    #[test]
    fn finish_flushes_queued_bytes_in_order() {
        let recorder = ChunkRecorder {
            chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let mut sender = TerminalOutputSender::spawn("test-tab", recorder.channel());
        sender.send(b"hello ".to_vec());
        sender.send("世界".as_bytes().to_vec());
        sender.finish();

        let chunks = recorder.chunks.lock().unwrap();
        let joined: Vec<u8> = chunks.concat();
        assert_eq!(joined, "hello 世界".as_bytes());
    }

    #[test]
    fn finish_waits_for_blocked_reader_sends() {
        let recorder = ChunkRecorder {
            chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let mut sender = TerminalOutputSender::spawn("test-tab", recorder.channel());
        // Far more than the queue capacity: some sends must block until the
        // worker drains them, proving finish() flushes everything in order.
        for i in 0..1000 {
            sender.send(vec![b'a' + (i % 26) as u8; 1024]);
        }
        sender.finish();

        let chunks = recorder.chunks.lock().unwrap();
        let joined: Vec<u8> = chunks.concat();
        assert_eq!(joined.len(), 1000 * 1024);
        for (index, byte) in joined.iter().enumerate() {
            let round = index / 1024;
            assert_eq!(
                *byte,
                b'a' + (round % 26) as u8,
                "byte {index} out of order"
            );
        }
    }

    #[test]
    fn send_after_finish_is_a_noop() {
        let recorder = ChunkRecorder {
            chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let mut sender = TerminalOutputSender::spawn("test-tab", recorder.channel());
        sender.send(b"first".to_vec());
        sender.finish();
        sender.send(b"dropped".to_vec());

        let chunks = recorder.chunks.lock().unwrap();
        let joined: Vec<u8> = chunks.concat();
        assert_eq!(joined, b"first");
    }

    #[test]
    fn rapid_messages_coalesce_into_fewer_chunks() {
        let recorder = ChunkRecorder {
            chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let mut sender = TerminalOutputSender::spawn("coalesce", recorder.channel());
        // All ten land inside one batching window, so they must reach the
        // webview as fewer IPC messages than reads.
        let mut expected = Vec::new();
        for i in 0..10 {
            let message = format!("message-{i:02} ").into_bytes();
            expected.extend_from_slice(&message);
            sender.send(message);
        }
        sender.finish();

        let chunks = recorder.chunks.lock().unwrap();
        assert!(
            chunks.len() < 10,
            "expected coalescing, got {} chunks",
            chunks.len()
        );
        assert_eq!(chunks.concat(), expected);
    }

    #[test]
    fn steady_trickle_flushes_without_waiting_for_disconnect() {
        let recorder = ChunkRecorder {
            chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let mut sender = TerminalOutputSender::spawn("trickle", recorder.channel());
        // Gaps below BATCH_INTERVAL keep a self-resetting recv_timeout from
        // ever firing; the window deadline must flush on its own anyway.
        let mut expected = Vec::new();
        for i in 0..10 {
            let message = format!("tick-{i} ").into_bytes();
            expected.extend_from_slice(&message);
            sender.send(message);
            std::thread::sleep(Duration::from_millis(2));
        }
        std::thread::sleep(BATCH_INTERVAL * 4);

        let delivered_before_finish = recorder.chunks.lock().unwrap().len();
        sender.finish();
        assert!(
            delivered_before_finish > 0,
            "trickle starved the webview until finish()"
        );
        assert_eq!(recorder.chunks.lock().unwrap().concat(), expected);
    }

    /// End-to-end: a fake PTY reader whose reads split multi-byte characters
    /// at arbitrary boundaries must surface as one ordered, valid UTF-8 stream
    /// through the batching sender, and the reader must only report
    /// termination after every byte has been delivered.
    #[test]
    fn reader_thread_delivers_batched_utf8_stream_in_order() {
        use std::io::Read;

        struct FragmentedReader {
            data: Vec<u8>,
            // Read sizes chosen to slice through the middle of sequences.
            sizes: Vec<usize>,
            cursor: usize,
            size_cursor: usize,
        }

        impl Read for FragmentedReader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.cursor >= self.data.len() {
                    return Ok(0);
                }
                let want = self.sizes[self.size_cursor % self.sizes.len()];
                self.size_cursor += 1;
                let take = want.min(buf.len()).min(self.data.len() - self.cursor);
                buf[..take].copy_from_slice(&self.data[self.cursor..self.cursor + take]);
                self.cursor += take;
                std::thread::sleep(Duration::from_millis(1));
                Ok(take)
            }
        }

        let mut expected = String::new();
        for round in 0..400 {
            expected.push_str(&format!("line-{round}: 中文测试 😀 tail\r\n"));
        }

        let received: std::sync::Arc<std::sync::Mutex<Vec<u8>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink_received = received.clone();
        let channel = TerminalOutput::new(Channel::new(move |body| {
            if let InvokeResponseBody::Raw(bytes) = body {
                sink_received.lock().unwrap().extend_from_slice(&bytes);
            }
            Ok(())
        }));

        let reader = FragmentedReader {
            data: expected.as_bytes().to_vec(),
            // 7 deliberately lands inside multi-byte sequences.
            sizes: vec![3, 7, 13, 1, 64],
            cursor: 0,
            size_cursor: 0,
        };

        let sender = std::sync::Mutex::new(Some(TerminalOutputSender::spawn("e2e", channel)));
        let outcome = super::super::pty::run_pty_reader(
            reader,
            |_| {},
            |chunk: &[u8]| {
                if let Some(sender) = sender.lock().unwrap().as_mut() {
                    sender.send(chunk.to_vec());
                }
            },
            || {
                if let Some(sender) = sender.lock().unwrap().as_mut() {
                    sender.finish();
                }
            },
        );

        assert_eq!(outcome, super::super::pty::ReaderOutcome::Terminated);

        let received = received.lock().unwrap().clone();
        let text = String::from_utf8(received).expect("delivered stream is valid UTF-8");
        assert_eq!(text, expected);
    }
}
