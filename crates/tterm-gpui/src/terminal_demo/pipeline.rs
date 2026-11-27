//! Parses output from an `AppServices` session and applies it to the GPUI model.
//!
//! Core batching sends UTF-8-safe chunks into a bounded queue. The parser
//! applies each chunk only while it belongs to the session generation it was
//! started for, answers the model's queries through the write callback, and
//! leaves at most one pending refresh for the view. The queue is bounded in
//! bytes as well as messages, so a sustained flood cannot grow memory between
//! the core sender and the parser.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tokio::sync::watch;
use tterm_core::terminal::ssh_query_handler::ordered::{OrderedQueries, Part};
use tterm_terminal::event::Intent;
use tterm_terminal::grid::WindowSize;
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::terminal::Terminal;

use super::clipboard::store::ClipboardWrites;
use super::intents::handle_intent;
use super::session::reports::Reports;

pub(super) mod mouse_guard;
use mouse_guard::MouseGuard;

mod ssh_replies;
use ssh_replies::DeferredReplies;

/// How many parser-sized chunks may wait for the parser before core applies
/// backpressure upstream.
pub(super) const QUEUED_CHUNKS: usize = 7;

/// About the most bytes the parser applies under one lock. Incoming chunks are
/// split at this size before they enter the queue, so a 512 KiB core batch
/// cannot hold the model lock for a whole IPC message.
pub(super) const BATCH_LIMIT: usize = 64 * 1024;

/// The most unparsed bytes in the channel, one blocked sender, and the batch
/// currently being applied.
///
/// Two extra chunks cover the sender waiting for a free slot and the parser's
/// in-flight batch. Sends are serialized so a second blocked sender cannot
/// reserve another chunk beyond this bound.
pub(super) const MAX_QUEUED_BYTES: usize = (QUEUED_CHUNKS + 2) * BATCH_LIMIT;

/// Hands chunks to the parser and records the queue's byte occupancy.
#[derive(Clone)]
pub(super) struct QueueSender {
    sender: SyncSender<QueuedChunk>,
    metrics: Arc<QueueMetrics>,
    send_lock: Arc<Mutex<()>>,
}

impl QueueSender {
    fn new(sender: SyncSender<QueuedChunk>, metrics: Arc<QueueMetrics>) -> Self {
        Self {
            sender,
            metrics,
            send_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Enqueues `bytes`, blocking once the bounded queue is full.
    ///
    /// A chunk larger than [`BATCH_LIMIT`] is split first. Every piece is
    /// counted before it reaches the channel, so the occupancy includes a
    /// sender blocked on a full queue. Clones serialize whole sends so only
    /// one sender can reserve a blocked piece against this budget.
    pub(super) fn send(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.send_inner(bytes, true)
    }

    /// Enqueues bytes synthesized by the client rather than received from the
    /// PTY. They are parsed with output but do not acknowledge `OutputFlow`.
    pub(super) fn send_synthetic(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.send_inner(bytes, false)
    }

    fn send_inner(&self, bytes: Vec<u8>, acknowledge_flow: bool) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        let _send_guard = self
            .send_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        #[cfg(test)]
        let _active_send = ActiveSendGuard::new(&self.metrics);
        for chunk in bytes.chunks(BATCH_LIMIT) {
            let length = chunk.len();
            self.metrics.queued(length);
            debug_assert!(
                self.queued_bytes() <= MAX_QUEUED_BYTES,
                "queued {} bytes exceeds {}",
                self.queued_bytes(),
                MAX_QUEUED_BYTES
            );
            let chunk = QueuedChunk {
                bytes: chunk.to_vec(),
                acknowledge_flow: acknowledge_flow.then_some(length),
            };
            if self.sender.send(chunk).is_err() {
                self.metrics.released(length);
                return Err("The GPUI output parser has closed".to_string());
            }
        }
        Ok(())
    }

    /// The bytes currently waiting in or immediately in front of the queue.
    pub(super) fn queued_bytes(&self) -> usize {
        self.metrics.queued.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(super) fn metrics_handle(&self) -> Arc<QueueMetrics> {
        Arc::clone(&self.metrics)
    }
}

/// One parser input plus the flow-control bytes it may acknowledge.
struct QueuedChunk {
    bytes: Vec<u8>,
    acknowledge_flow: Option<usize>,
}

#[cfg(test)]
struct ActiveSendGuard<'a> {
    metrics: &'a QueueMetrics,
}

#[cfg(test)]
impl<'a> ActiveSendGuard<'a> {
    fn new(metrics: &'a QueueMetrics) -> Self {
        metrics.sender_entered();
        Self { metrics }
    }
}

#[cfg(test)]
impl Drop for ActiveSendGuard<'_> {
    fn drop(&mut self) {
        self.metrics.sender_left();
    }
}

/// Counters shared by one session's queue and parser.
#[derive(Default)]
pub(super) struct QueueMetrics {
    queued: AtomicUsize,
    max_queued: AtomicUsize,
    sent: AtomicU64,
    applied: AtomicU64,
    acked: AtomicU64,
    batches: AtomicU64,
    #[cfg(test)]
    active_sends: AtomicUsize,
    #[cfg(test)]
    max_active_sends: AtomicUsize,
}

impl QueueMetrics {
    fn queued(&self, bytes: usize) {
        let now = self.queued.fetch_add(bytes, Ordering::AcqRel) + bytes;
        self.max_queued.fetch_max(now, Ordering::AcqRel);
        self.sent
            .fetch_add(u64::try_from(bytes).unwrap_or(u64::MAX), Ordering::Relaxed);
    }

    fn released(&self, bytes: usize) {
        self.queued.fetch_sub(bytes, Ordering::AcqRel);
    }

    fn parsed(&self, bytes: usize, applied: bool) {
        if applied {
            self.applied
                .fetch_add(u64::try_from(bytes).unwrap_or(u64::MAX), Ordering::Relaxed);
        }
        self.acked
            .fetch_add(u64::try_from(bytes).unwrap_or(u64::MAX), Ordering::Relaxed);
        self.batches.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(test)]
    fn sender_entered(&self) {
        let now = self.active_sends.fetch_add(1, Ordering::AcqRel) + 1;
        self.max_active_sends.fetch_max(now, Ordering::AcqRel);
    }

    #[cfg(test)]
    fn sender_left(&self) {
        self.active_sends.fetch_sub(1, Ordering::AcqRel);
    }

    /// The bytes accepted by the queue but not yet parsed.
    #[cfg(test)]
    pub(super) fn queued_bytes(&self) -> usize {
        self.queued.load(Ordering::Acquire)
    }

    /// The high-water mark of [`QueueMetrics::queued_bytes`].
    #[cfg(test)]
    pub(super) fn max_queued_bytes(&self) -> usize {
        self.max_queued.load(Ordering::Acquire)
    }

    /// The bytes accepted by the queue.
    #[cfg(test)]
    pub(super) fn sent_bytes(&self) -> u64 {
        self.sent.load(Ordering::Acquire)
    }

    /// The bytes the parser applied to this session's model.
    #[cfg(test)]
    pub(super) fn applied_bytes(&self) -> u64 {
        self.applied.load(Ordering::Acquire)
    }

    /// The bytes the parser has processed, including client-synthesized output.
    #[cfg(test)]
    pub(super) fn acknowledged_bytes(&self) -> u64 {
        self.acked.load(Ordering::Acquire)
    }

    /// The number of parser batches.
    #[cfg(test)]
    pub(super) fn batches(&self) -> u64 {
        self.batches.load(Ordering::Acquire)
    }

    /// The most senders that were simultaneously inside a reserved send.
    #[cfg(test)]
    pub(super) fn max_active_sends(&self) -> usize {
        self.max_active_sends.load(Ordering::Acquire)
    }
}

/// The one-slot refresh signal between the parser and the view.
///
/// A flooded parser calls [`RefreshSignal::notify`] many times, but only the
/// first call between two acknowledgements touches the watch channel. The
/// view acknowledges when it starts handling a frame, so sustained output
/// cannot enqueue an unbounded stream of UI wakes.
pub(super) struct RefreshSignal {
    pending: AtomicBool,
    sends: AtomicU64,
    generation: watch::Sender<u64>,
}

impl RefreshSignal {
    pub(super) fn new() -> Self {
        let (generation, _receiver) = watch::channel(0_u64);
        Self {
            pending: AtomicBool::new(false),
            sends: AtomicU64::new(0),
            generation,
        }
    }

    /// Wakes the view once until it acknowledges this refresh.
    pub(super) fn notify(&self) {
        if self.pending.swap(true, Ordering::AcqRel) {
            return;
        }
        self.sends.fetch_add(1, Ordering::Relaxed);
        self.generation
            .send_modify(|generation| *generation = generation.wrapping_add(1));
    }

    /// Marks the pending refresh as consumed, so the next output can wake the
    /// view again.
    pub(super) fn acknowledge(&self) {
        self.pending.store(false, Ordering::Release);
    }

    pub(super) fn subscribe(&self) -> watch::Receiver<u64> {
        let receiver = self.generation.subscribe();
        // A view subscribes only after its model has been constructed, so it
        // already sees output parsed while no receiver existed. Clearing the
        // slot here prevents that output's pending bit from suppressing the
        // next live notification.
        self.pending.store(false, Ordering::Release);
        receiver
    }

    #[cfg(test)]
    pub(super) fn sends(&self) -> u64 {
        self.sends.load(Ordering::Acquire)
    }
}

/// What the parser thread shares with the window.
#[derive(Clone)]
pub(super) struct SessionState {
    /// The terminal model the parser writes and the view draws.
    pub terminal: Arc<Terminal>,
    /// The grid and cell size of the last resize, for `CSI 14 t` replies.
    pub size: Arc<Mutex<WindowSize>>,
    /// The latest title the program set that the window has not shown yet.
    pub title: Arc<Mutex<Option<String>>>,
    /// The palette the attached view draws with.
    ///
    /// Color-query replies read it on the parser thread, so a theme change
    /// has to update this and the view together to keep the answer and the
    /// pixels in step. The model itself never stores theme colors.
    pub palette: Arc<Mutex<TerminalPalette>>,
    /// The bounded OSC 52 write slot consumed on the UI thread.
    pub clipboard: Arc<ClipboardWrites>,
    /// The generation whose output this parser may still apply.
    pub session_nonce: u32,
    /// Cleared when the view or tab releases this session.
    pub alive: Arc<AtomicBool>,
    /// Whether a view currently shows this session.
    pub visible: Arc<AtomicBool>,
    /// The coalesced frame wake-up.
    pub refresh: Arc<RefreshSignal>,
    /// Prompt, directory and agent reports parsed from applied output.
    pub reports: Arc<Reports>,
}

impl SessionState {
    /// Applies output only from the current generation of a live session.
    ///
    /// A parser can still hold a queued batch when its session closes or a
    /// reconnect starts a newer nonce. That batch must not reach the model or
    /// wake a view attached to the new session.
    pub(super) fn apply(&self, session_nonce: u32, bytes: &[u8]) -> bool {
        if session_nonce != self.session_nonce || !self.alive.load(Ordering::Acquire) {
            return false;
        }
        self.terminal.write(bytes);
        self.reports.observe(bytes);
        true
    }

    /// The palette color-query replies resolve against.
    pub(super) fn palette(&self) -> TerminalPalette {
        self.palette
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Replaces the palette color-query replies resolve against.
    ///
    /// The caller owns the matching repaint of the attached view; this only
    /// updates what the parser answers, so the model and its output stay
    /// untouched.
    pub(super) fn set_palette(&self, palette: TerminalPalette) {
        *self.palette.lock().unwrap_or_else(PoisonError::into_inner) = palette;
    }

    /// Wakes the view only when it can actually draw this session.
    fn publish(&self) {
        if self.visible.load(Ordering::Acquire) && self.alive.load(Ordering::Acquire) {
            self.refresh.notify();
        }
    }
}

/// The queue that carries core output into the parser.
pub(super) struct Pipeline {
    pub queue: QueueSender,
    pub parser: JoinHandle<()>,
}

type WriteBytes = Arc<dyn Fn(Vec<u8>) + Send + Sync>;
type AcknowledgeBytes = Arc<dyn Fn(usize) + Send + Sync>;

/// Starts the parser with the core output write and acknowledgement paths.
pub(super) fn start(
    state: SessionState,
    write: WriteBytes,
    acknowledge: AcknowledgeBytes,
    guard: Option<Arc<MouseGuard>>,
    ssh_queries: Option<OrderedQueries>,
) -> Result<Pipeline, String> {
    if ssh_queries.is_some() {
        state.terminal.enable_ordered_replies();
    }
    let metrics = Arc::new(QueueMetrics::default());
    let (sender, queued) = mpsc::sync_channel(QUEUED_CHUNKS);
    let queue = QueueSender::new(sender, Arc::clone(&metrics));
    let parser = spawn_parser(
        queued,
        Arc::clone(&metrics),
        ParserOutput {
            state,
            write,
            guard,
            ssh_queries,
            ssh_replies: DeferredReplies::default(),
            sync_deadline: None,
        },
        acknowledge,
    )?;
    Ok(Pipeline { queue, parser })
}

fn spawn_parser(
    queued: Receiver<QueuedChunk>,
    metrics: Arc<QueueMetrics>,
    mut output: ParserOutput,
    acknowledge: AcknowledgeBytes,
) -> Result<JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("gpui-pty-parser".to_string())
        .spawn(move || {
            loop {
                let now = Instant::now();
                if output.sync_deadline.is_some_and(|deadline| deadline <= now) {
                    output.flush_expired_synchronized_output();
                }
                let sync_wait = output.sync_deadline.map(|deadline| {
                    deadline
                        .saturating_duration_since(now)
                        .max(Duration::from_millis(1))
                });
                let guard_wait = output
                    .guard
                    .as_ref()
                    .and_then(|guard| guard.timeout_at(now));
                let wait = match (sync_wait, guard_wait) {
                    (Some(sync), Some(guard)) => Some(sync.min(guard)),
                    (Some(sync), None) => Some(sync),
                    (None, Some(guard)) => Some(guard),
                    (None, None) => None,
                };
                let chunk = match wait {
                    Some(wait) => match queued.recv_timeout(wait) {
                        Ok(chunk) => Some(chunk),
                        Err(RecvTimeoutError::Timeout) => {
                            output.on_timeout();
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => None,
                    },
                    None => queued.recv().ok(),
                };
                let Some(chunk) = chunk else {
                    break;
                };
                let (batch, acknowledge_flow) = chunk.into_batch();
                let applied = output.apply(&batch, acknowledge_flow != 0);
                if acknowledge_flow != 0 {
                    acknowledge(acknowledge_flow);
                }
                metrics.parsed(batch.len(), applied);
                metrics.released(batch.len());
            }
        })
        .map_err(|error| format!("Failed to start the GPUI output parser: {error}"))
}

impl QueuedChunk {
    fn into_batch(self) -> (Vec<u8>, usize) {
        (self.bytes, self.acknowledge_flow.unwrap_or_default())
    }
}

struct ParserOutput {
    state: SessionState,
    write: WriteBytes,
    guard: Option<Arc<MouseGuard>>,
    ssh_queries: Option<OrderedQueries>,
    ssh_replies: DeferredReplies,
    sync_deadline: Option<Instant>,
}

impl ParserOutput {
    /// Emits all replies in parse order before the caller releases flow credit.
    fn apply(&mut self, bytes: &[u8], transport: bool) -> bool {
        let deadline = {
            let Self {
                state,
                write,
                guard,
                ssh_queries,
                ssh_replies,
                ..
            } = self;
            if !state.alive.load(Ordering::Acquire) {
                return false;
            }
            let mut applier = Applier {
                clipboard_epoch: state.clipboard.epoch(),
                state,
                write,
                guard,
                deferred: ssh_replies,
            };
            if let Some(queries) = ssh_queries.as_mut().filter(|_| transport) {
                queries.feed(bytes, |part| match part {
                    Part::Display(bytes) => applier.display(bytes),
                    Part::Reply(reply) if state.alive.load(Ordering::Acquire) => {
                        applier.reply(reply);
                    }
                    Part::Reply(_) => {}
                });
            } else {
                applier.display(bytes);
            }
            state.publish();
            state.terminal.synchronized_output_deadline()
        };
        self.sync_deadline = deadline;
        true
    }

    /// Runs whatever became due while the queue was idle.
    fn on_timeout(&mut self) {
        self.flush_expired_synchronized_output();
        if let Some(guard) = &self.guard {
            guard.settle();
        }
    }

    /// Applies the model's expired synchronized buffer and emits its replies.
    ///
    /// ESU normally does this. A program that never sends it leaves the screen
    /// hidden, so the parser aborts the update at Alacritty's deadline and
    /// writes the model replies that were waiting inside it.
    fn flush_expired_synchronized_output(&mut self) {
        if !self.state.alive.load(Ordering::Acquire) {
            self.sync_deadline = None;
            return;
        }
        if self.state.terminal.flush_expired_synchronized_output() {
            let mut applier = Applier {
                clipboard_epoch: self.state.clipboard.epoch(),
                state: &self.state,
                write: &self.write,
                guard: &self.guard,
                deferred: &mut self.ssh_replies,
            };
            applier.flush_intents();
            self.state.publish();
        }
        // A no-op flush can mean the model already closed the block. Refresh
        // even then so a stale deadline cannot keep the queue waking.
        self.sync_deadline = self.state.terminal.synchronized_output_deadline();
    }
}

/// Applies output and resolves intents for one parser batch.
struct Applier<'a> {
    clipboard_epoch: u64,
    state: &'a SessionState,
    write: &'a WriteBytes,
    guard: &'a Option<Arc<MouseGuard>>,
    deferred: &'a mut DeferredReplies,
}

impl Applier<'_> {
    /// Feeds one display slice to the model and emits what it produced.
    fn display(&mut self, bytes: &[u8]) {
        let applied = match self.guard.as_ref() {
            Some(guard) => guard.apply_batch(bytes),
            None => self.state.apply(self.state.session_nonce, bytes),
        };
        if applied {
            self.flush_intents();
        }
    }

    /// Emits the model's intents, replacing an ordering marker with the fixed
    /// reply that waits at that point in the stream.
    fn flush_intents(&mut self) {
        for intent in self.state.terminal.drain_intents() {
            self.intent(intent);
        }
    }

    fn intent(&mut self, intent: Intent) {
        if let Intent::OrderedReply(order) = intent {
            if let Some(reply) = self.deferred.take(order) {
                (self.write)(reply);
            }
            return;
        }
        handle_intent(intent, self.state, self.clipboard_epoch, |reply| {
            (self.write)(reply)
        });
    }

    /// Writes a fixed reply, or defers it while the parser holds a
    /// synchronized update whose model replies have not been produced yet.
    fn reply(&mut self, reply: &[u8]) {
        if self.state.terminal.has_synchronized_output() {
            self.defer(reply);
        } else {
            (self.write)(reply.to_vec());
        }
    }

    /// Inserts a marker at a fixed reply's position and anchors the reply to it.
    fn defer(&mut self, reply: &[u8]) {
        let order = self.deferred.defer(reply.to_vec());
        self.state.terminal.write_ordered_reply(order);
        self.flush_intents();
    }
}

#[cfg(test)]
mod p2_12_bench;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod throughput_tests;
