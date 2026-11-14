//! Parses output from an `AppServices` session and applies it to the GPUI model.
//!
//! Core batching sends UTF-8-safe chunks into a bounded queue. The parser
//! applies each chunk, answers the model's queries through `AppServices`, and
//! leaves at most one pending wake for the view.

use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use tterm_core::prompts::SessionKey;
use tterm_core::services::AppServices;
use tterm_core::terminal::io_batcher::OutputFlow;
use tterm_terminal::grid::WindowSize;
use tterm_terminal::terminal::Terminal;

use super::intents::handle_intent;

/// How many output chunks may wait for the parser before core applies
/// backpressure upstream.
pub(super) const QUEUED_CHUNKS: usize = 8;

/// About the most bytes the parser applies under one lock. It takes whatever
/// is queued up to this limit and never waits to fill a batch.
const BATCH_LIMIT: usize = 64 * 1024;

/// What the parser thread shares with the window.
#[derive(Clone)]
pub(super) struct SessionState {
    /// The terminal model the parser writes and the view draws.
    pub terminal: Arc<Terminal>,
    /// The grid and cell size of the last resize, for `CSI 14 t` replies.
    pub size: Arc<Mutex<WindowSize>>,
    /// The latest title the program set that the window has not shown yet.
    pub title: Arc<Mutex<Option<String>>>,
}

/// The queue that carries core output into the parser.
pub(super) struct Pipeline {
    pub queue: SyncSender<Vec<u8>>,
    pub parser: JoinHandle<()>,
}

/// Starts the parser with the core output acknowledgement account.
pub(super) fn start(
    state: SessionState,
    services: Arc<AppServices>,
    key: SessionKey,
    flow: Arc<OutputFlow>,
    wake: tokio::sync::mpsc::Sender<()>,
) -> Result<Pipeline, String> {
    let (queue, queued) = mpsc::sync_channel(QUEUED_CHUNKS);
    let parser = spawn_parser(queued, state, services, key, flow, wake)?;
    Ok(Pipeline { queue, parser })
}

fn spawn_parser(
    queued: Receiver<Vec<u8>>,
    state: SessionState,
    services: Arc<AppServices>,
    key: SessionKey,
    flow: Arc<OutputFlow>,
    wake: tokio::sync::mpsc::Sender<()>,
) -> Result<JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("gpui-pty-parser".to_string())
        .spawn(move || {
            while let Ok(mut batch) = queued.recv() {
                while batch.len() < BATCH_LIMIT {
                    match queued.try_recv() {
                        Ok(more) => batch.extend_from_slice(&more),
                        Err(_) => break,
                    }
                }
                let length = batch.len();
                state.terminal.write(&batch);
                for intent in state.terminal.drain_intents() {
                    handle_intent(intent, &state, |bytes| {
                        let _ = services.write_pty(&key, &bytes);
                    });
                }
                flow.ack(length);
                let _ = wake.try_send(());
            }
        })
        .map_err(|error| format!("Failed to start the GPUI output parser: {error}"))
}
