//! Each session's log is written on its own thread, so a slow or stalled disk
//! never holds up the terminal. The terminal side only queues what happened
//! and when; the thread turns that into log lines and writes them out.

use super::writer::{SessionWriter, Stamp};
use super::{emit_error, SessionLogState};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// Output a session may have waiting to be written. Past this the disk is not
/// keeping up, so output is dropped, and the gap noted in the log, rather than
/// holding up the terminal.
const MAX_QUEUED_BYTES: usize = 16 * 1024 * 1024;
/// Buffered log lines are written out at least this often.
const FLUSH_INTERVAL: Duration = Duration::from_secs(1);
/// Each time a session has logged this much, old logs are checked against
/// the retention settings.
const MAINTENANCE_BYTES: u64 = 8 * 1024 * 1024;
/// How long a reader waits for a log to write out what it has buffered.
const FLUSH_WAIT: Duration = Duration::from_secs(2);

/// Threads of logs that were asked to close and may still be writing.
static CLOSING: LazyLock<Mutex<Vec<JoinHandle<()>>>> = LazyLock::new(Default::default);

enum Message {
    Data {
        direction: &'static str,
        data: Vec<u8>,
        at: Stamp,
        /// Output dropped just before this, while the queue was full.
        dropped_before: u64,
    },
    Event {
        name: &'static str,
        detail: serde_json::Value,
        at: Stamp,
    },
    Resize {
        rows: u16,
        cols: u16,
        at: Stamp,
    },
    Flush(Sender<()>),
    Close {
        event: &'static str,
        at: Stamp,
    },
}

#[derive(Default)]
struct Queue {
    bytes: AtomicUsize,
    dropped: AtomicU64,
}

/// The terminal's side of a session's log.
pub(super) struct LogHandle {
    /// The log's id in the log library.
    pub(super) id: String,
    sender: Sender<Message>,
    queue: Arc<Queue>,
    thread: JoinHandle<()>,
}

impl LogHandle {
    /// Starts the thread that writes `writer`'s log.
    pub(super) fn spawn(
        app: &AppHandle,
        tab_id: &str,
        writer: SessionWriter,
    ) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        let queue = Arc::new(Queue::default());
        let id = writer.id.clone();
        let thread = std::thread::Builder::new()
            .name("tterm-session-log".to_string())
            .spawn({
                let app = app.clone();
                let tab_id = tab_id.to_string();
                let queue = queue.clone();
                move || run(&app, &tab_id, writer, &receiver, &queue)
            })
            .map_err(|err| format!("Failed to start writing the terminal log: {err}"))?;
        Ok(Self {
            id,
            sender,
            queue,
            thread,
        })
    }

    /// Queues output or input, unless too much is already waiting.
    pub(super) fn data(&self, direction: &'static str, data: &[u8], at: Stamp) {
        if data.is_empty() {
            return;
        }
        let queued = self.queue.bytes.fetch_add(data.len(), Ordering::AcqRel) + data.len();
        if queued > MAX_QUEUED_BYTES {
            self.queue.bytes.fetch_sub(data.len(), Ordering::AcqRel);
            self.queue
                .dropped
                .fetch_add(data.len() as u64, Ordering::AcqRel);
            return;
        }
        let message = Message::Data {
            direction,
            data: data.to_vec(),
            at,
            dropped_before: self.queue.dropped.swap(0, Ordering::AcqRel),
        };
        if self.sender.send(message).is_err() {
            self.queue.bytes.fetch_sub(data.len(), Ordering::AcqRel);
        }
    }

    pub(super) fn event(&self, name: &'static str, detail: serde_json::Value, at: Stamp) {
        let _ = self.sender.send(Message::Event { name, detail, at });
    }

    pub(super) fn resize(&self, rows: u16, cols: u16, at: Stamp) {
        let _ = self.sender.send(Message::Resize { rows, cols, at });
    }

    /// Asks the thread to write out what it has buffered; the receiver hears
    /// once it has. See `wait_flushed`.
    pub(super) fn request_flush(&self) -> Option<Receiver<()>> {
        let (done, flushed) = mpsc::channel();
        self.sender.send(Message::Flush(done)).ok()?;
        Some(flushed)
    }

    /// Ends the log with `event` once what is queued is written. Does not
    /// wait for the thread; `wait_for_closing` does.
    pub(super) fn close(self, event: &'static str) {
        let _ = self.sender.send(Message::Close {
            event,
            at: Stamp::now(),
        });
        let mut closing = match CLOSING.lock() {
            Ok(closing) => closing,
            Err(poisoned) => poisoned.into_inner(),
        };
        closing.retain(|thread| !thread.is_finished());
        closing.push(self.thread);
    }
}

/// Waits a short while for a requested flush.
pub(super) fn wait_flushed(flushed: &Receiver<()>) {
    let _ = flushed.recv_timeout(FLUSH_WAIT);
}

/// Waits up to `limit` for the logs asked to close to finish writing.
pub(super) fn wait_for_closing(limit: Duration) {
    let threads = match CLOSING.lock() {
        Ok(mut closing) => std::mem::take(&mut *closing),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    };
    let deadline = Instant::now() + limit;
    for thread in threads {
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Reports write errors without repeating the same one for every chunk while
/// the disk stays unwritable.
struct Reporter<'a> {
    app: &'a AppHandle,
    tab_id: &'a str,
    last: Option<String>,
}

impl Reporter<'_> {
    fn report(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => self.last = None,
            Err(err) => self.error(err),
        }
    }

    fn error(&mut self, err: String) {
        if self.last.as_ref() == Some(&err) {
            return;
        }
        self.app.state::<SessionLogState>().set_error(err.clone());
        emit_error(self.app, self.tab_id, &err);
        self.last = Some(err);
    }
}

fn record_gap(writer: &mut SessionWriter, dropped: u64, at: Stamp) -> Result<(), String> {
    if dropped == 0 {
        return Ok(());
    }
    writer.record_event(
        "log_gap",
        serde_json::json!({ "droppedBytes": dropped }),
        at,
    )
}

fn run(
    app: &AppHandle,
    tab_id: &str,
    mut writer: SessionWriter,
    receiver: &Receiver<Message>,
    queue: &Queue,
) {
    let mut reporter = Reporter {
        app,
        tab_id,
        last: None,
    };
    let mut last_flush = Instant::now();
    let mut since_maintenance = 0u64;
    loop {
        let message = match receiver.recv_timeout(FLUSH_INTERVAL) {
            Ok(message) => Some(message),
            Err(RecvTimeoutError::Timeout) => None,
            // The session went away without closing its log.
            Err(RecvTimeoutError::Disconnected) => Some(Message::Close {
                event: "session_end",
                at: Stamp::now(),
            }),
        };
        match message {
            None => {}
            Some(Message::Data {
                direction,
                data,
                at,
                dropped_before,
            }) => {
                queue.bytes.fetch_sub(data.len(), Ordering::AcqRel);
                since_maintenance += data.len() as u64;
                reporter.report(
                    record_gap(&mut writer, dropped_before, at)
                        .and_then(|()| writer.record_bytes(direction, &data, at)),
                );
            }
            Some(Message::Event { name, detail, at }) => {
                reporter.report(writer.record_event(name, detail, at));
            }
            Some(Message::Resize { rows, cols, at }) => {
                reporter.report(writer.resize(rows, cols, at));
            }
            Some(Message::Flush(done)) => {
                if let Err(err) = writer.flush() {
                    reporter.error(err);
                }
                last_flush = Instant::now();
                let _ = done.send(());
            }
            Some(Message::Close { event, at }) => {
                let dropped = queue.dropped.swap(0, Ordering::AcqRel);
                reporter.report(
                    record_gap(&mut writer, dropped, at)
                        .and_then(|()| writer.close_with(event, at)),
                );
                break;
            }
        }
        if last_flush.elapsed() >= FLUSH_INTERVAL {
            if let Err(err) = writer.flush() {
                reporter.error(err);
            }
            last_flush = Instant::now();
        }
        if since_maintenance >= MAINTENANCE_BYTES {
            since_maintenance = 0;
            app.state::<SessionLogState>()
                .schedule_maintenance(app, false);
        }
    }
    app.state::<SessionLogState>()
        .schedule_maintenance(app, false);
}
