//! Parses output from an `AppServices` session and applies it to the GPUI model.
//!
//! Core batching sends UTF-8-safe chunks into a bounded queue. The parser
//! applies each chunk only while it belongs to the session generation it was
//! started for, answers the model's queries through the write callback, and
//! leaves at most one pending refresh for the view. The queue is bounded in
//! bytes as well as messages, so a sustained flood cannot grow memory between
//! the core sender and the parser.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use tokio::sync::watch;
use tterm_terminal::grid::WindowSize;
use tterm_terminal::terminal::Terminal;

use super::intents::handle_intent;

/// How many parser-sized chunks may wait for the parser before core applies
/// backpressure upstream.
pub(super) const QUEUED_CHUNKS: usize = 8;

/// About the most bytes the parser applies under one lock. Incoming chunks are
/// split at this size before they enter the queue, so a 512 KiB core batch
/// cannot hold the model lock for a whole IPC message.
pub(super) const BATCH_LIMIT: usize = 64 * 1024;

/// The most bytes that may wait in or immediately in front of the queue.
///
/// One extra chunk covers a sender that has reserved its chunk and is waiting
/// for a free queue slot; the queued bytes themselves are
/// [`QUEUED_CHUNKS`] × [`BATCH_LIMIT`].
pub(super) const MAX_QUEUED_BYTES: usize = (QUEUED_CHUNKS + 1) * BATCH_LIMIT;

/// Hands chunks to the parser and records the queue's byte occupancy.
#[derive(Clone)]
pub(super) struct QueueSender {
    sender: SyncSender<Vec<u8>>,
    metrics: Arc<QueueMetrics>,
}

impl QueueSender {
    fn new(sender: SyncSender<Vec<u8>>, metrics: Arc<QueueMetrics>) -> Self {
        Self { sender, metrics }
    }

    /// Enqueues `bytes`, blocking once the bounded queue is full.
    ///
    /// A chunk larger than [`BATCH_LIMIT`] is split first. Every piece is
    /// counted before it reaches the channel, so the occupancy includes a
    /// sender blocked on a full queue.
    pub(super) fn send(&self, bytes: Vec<u8>) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        for chunk in bytes.chunks(BATCH_LIMIT) {
            self.metrics.queued(chunk.len());
            if self.sender.send(chunk.to_vec()).is_err() {
                self.metrics.released(chunk.len());
                return Err("The GPUI output parser has closed".to_string());
            }
            debug_assert!(self.queued_bytes() <= MAX_QUEUED_BYTES);
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

/// Counters shared by one session's queue and parser.
#[derive(Default)]
pub(super) struct QueueMetrics {
    queued: AtomicUsize,
    max_queued: AtomicUsize,
    sent: AtomicU64,
    acked: AtomicU64,
    batches: AtomicU64,
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

    fn parsed(&self, bytes: usize) {
        self.acked
            .fetch_add(u64::try_from(bytes).unwrap_or(u64::MAX), Ordering::Relaxed);
        self.batches.fetch_add(1, Ordering::Relaxed);
    }

    /// The bytes that were queued and not yet taken by the parser.
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

    /// The bytes the parser has processed and acknowledged.
    #[cfg(test)]
    pub(super) fn acknowledged_bytes(&self) -> u64 {
        self.acked.load(Ordering::Acquire)
    }

    /// The number of parser batches.
    #[cfg(test)]
    pub(super) fn batches(&self) -> u64 {
        self.batches.load(Ordering::Acquire)
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
    /// The generation whose output this parser may still apply.
    pub session_nonce: u32,
    /// Cleared when the view or tab releases this session.
    pub alive: Arc<AtomicBool>,
    /// Whether a view currently shows this session.
    pub visible: Arc<AtomicBool>,
    /// The coalesced frame wake-up.
    pub refresh: Arc<RefreshSignal>,
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
        true
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
) -> Result<Pipeline, String> {
    let metrics = Arc::new(QueueMetrics::default());
    let (sender, queued) = mpsc::sync_channel(QUEUED_CHUNKS);
    let queue = QueueSender::new(sender, Arc::clone(&metrics));
    let parser = spawn_parser(queued, Arc::clone(&metrics), state, write, acknowledge)?;
    Ok(Pipeline { queue, parser })
}

fn spawn_parser(
    queued: Receiver<Vec<u8>>,
    metrics: Arc<QueueMetrics>,
    state: SessionState,
    write: WriteBytes,
    acknowledge: AcknowledgeBytes,
) -> Result<JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("gpui-pty-parser".to_string())
        .spawn(move || {
            let mut pending: Option<Vec<u8>> = None;
            loop {
                let mut batch = match pending.take() {
                    Some(batch) => batch,
                    None => match receive(&queued, &metrics) {
                        Some(batch) => batch,
                        None => break,
                    },
                };
                // Small chunks from a trickle are combined up to one batch.
                // The sender already caps each piece at BATCH_LIMIT, so this
                // loop never grows a batch beyond that limit.
                while batch.len() < BATCH_LIMIT {
                    match queued.try_recv() {
                        Ok(more) => {
                            metrics.released(more.len());
                            if batch.len() + more.len() <= BATCH_LIMIT {
                                batch.extend_from_slice(&more);
                            } else {
                                pending = Some(more);
                                break;
                            }
                        }
                        Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                    }
                }
                apply_and_release(&state, &metrics, &write, &acknowledge, &mut batch);
            }
        })
        .map_err(|error| format!("Failed to start the GPUI output parser: {error}"))
}

fn receive(queued: &Receiver<Vec<u8>>, metrics: &QueueMetrics) -> Option<Vec<u8>> {
    let batch = queued.recv().ok()?;
    metrics.released(batch.len());
    Some(batch)
}

fn apply_and_release(
    state: &SessionState,
    metrics: &QueueMetrics,
    write: &WriteBytes,
    acknowledge: &AcknowledgeBytes,
    batch: &mut Vec<u8>,
) {
    let length = batch.len();
    if state.apply(state.session_nonce, batch) {
        for intent in state.terminal.drain_intents() {
            handle_intent(intent, state, |bytes| write(bytes));
        }
        state.publish();
    }
    // The acknowledgement follows the model write and its intents. A batch
    // rejected because the generation closed is acknowledged too, because the
    // reader must not stay blocked on bytes that will never be drawn.
    acknowledge(length);
    metrics.parsed(length);
    batch.clear();
}

#[cfg(test)]
mod tests;
