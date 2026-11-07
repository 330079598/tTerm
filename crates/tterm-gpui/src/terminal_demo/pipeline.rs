//! A session's output pipeline: the reader, parser and writer threads of
//! terminal-design.md section 3.
//!
//! The reader only reads and queues. The queue is bounded, so when the parser
//! falls behind the reader blocks and the PTY holds the program back, instead
//! of bytes piling up in memory. The parser applies what is queued under the
//! model's fair lock, answers the model's queries and leaves at most one wake
//! for the view. The writer is the only code that writes to the PTY, so user
//! input and query replies reach the program in the order they were sent.

use std::io::{ErrorKind, Read, Write};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use tterm_terminal::grid::WindowSize;
use tterm_terminal::terminal::Terminal;

use super::intents::handle_intent;

/// How many reads may wait for the parser before the reader blocks.
pub(super) const QUEUED_READS: usize = 8;

/// The size of one read from the PTY.
pub(super) const READ_SIZE: usize = 8 * 1024;

/// About the most bytes the parser applies under one lock. It takes whatever
/// is queued up to this limit and never waits to fill a batch, so an echo is
/// drawn as soon as it arrives.
const BATCH_LIMIT: usize = 64 * 1024;

/// A message for the writer thread.
pub(super) enum WriteMessage {
    /// Bytes for the program: user input or a query reply.
    Bytes(Vec<u8>),
    /// Stop writing. The view's input callback keeps a sender for as long as
    /// the view lives, so the writer cannot wait for every sender to go.
    Close,
}

/// What the parser thread shares with the window.
#[derive(Clone)]
pub(super) struct SessionState {
    /// The terminal model the parser writes and the view draws.
    pub terminal: Arc<Terminal>,
    /// The grid and cell size of the last resize, for `CSI 14 t` replies.
    /// Taking a snapshot to learn them would consume the damage the view
    /// needs to redraw the changed rows.
    pub size: Arc<Mutex<WindowSize>>,
    /// The latest title the program set that the window has not shown yet.
    pub title: Arc<Mutex<Option<String>>>,
}

/// The pipeline's threads and the channel ends the session keeps.
pub(super) struct Pipeline {
    /// Queues bytes behind the program's output, such as the exit notice.
    /// The parser runs until every sender of this queue is gone.
    pub queue: SyncSender<Vec<u8>>,
    /// The ordered write channel for user input.
    pub writes: Sender<WriteMessage>,
    /// Holds at most one pending "new output" wake for the view.
    pub wake: async_channel::Receiver<()>,
    /// Ends when the reader sees end of file or an error.
    pub reader: JoinHandle<()>,
    /// Ends when the reader and every other queue sender are gone.
    pub parser: JoinHandle<()>,
    /// Ends on [`WriteMessage::Close`] or a failed write.
    pub writer: JoinHandle<()>,
}

/// Starts the threads over a PTY's reader and writer, or over any stream in
/// tests.
pub(super) fn start(
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    state: SessionState,
) -> Pipeline {
    let (queue, queued) = mpsc::sync_channel(QUEUED_READS);
    let (writes, messages) = mpsc::channel();
    let (wake_sender, wake) = async_channel::bounded(1);
    Pipeline {
        reader: spawn_reader(reader, queue.clone()),
        parser: spawn_parser(queued, state, writes.clone(), wake_sender),
        writer: spawn_writer(writer, messages),
        queue,
        writes,
        wake,
    }
}

fn spawn_reader(mut reader: Box<dyn Read + Send>, queue: SyncSender<Vec<u8>>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = vec![0u8; READ_SIZE];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    if queue.send(buffer[..count].to_vec()).is_err() {
                        break;
                    }
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    })
}

fn spawn_parser(
    queued: Receiver<Vec<u8>>,
    state: SessionState,
    writes: Sender<WriteMessage>,
    wake: async_channel::Sender<()>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(mut batch) = queued.recv() {
            while batch.len() < BATCH_LIMIT {
                match queued.try_recv() {
                    Ok(more) => batch.extend_from_slice(&more),
                    Err(_) => break,
                }
            }
            state.terminal.write(&batch);
            // Drained every batch, so intents never pile up in the model.
            for intent in state.terminal.drain_intents() {
                handle_intent(intent, &state, &writes);
            }
            // A full channel already holds a wake the view has not taken.
            let _ = wake.try_send(());
        }
    })
}

fn spawn_writer(
    mut writer: Box<dyn Write + Send>,
    messages: Receiver<WriteMessage>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(WriteMessage::Bytes(bytes)) = messages.recv() {
            if writer
                .write_all(&bytes)
                .and_then(|()| writer.flush())
                .is_err()
            {
                break;
            }
        }
    })
}
