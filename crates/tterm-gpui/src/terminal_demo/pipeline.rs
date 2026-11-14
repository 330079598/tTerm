//! Parses output from an `AppServices` session and applies it to the GPUI model.
//!
//! Core batching sends UTF-8-safe chunks into a bounded queue. The parser
//! applies each chunk only while it belongs to the session generation it was
//! started for, answers the model's queries through `AppServices`, and leaves
//! at most one pending wake for the view.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use tokio::sync::watch;
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
    /// The generation whose output this parser may still apply.
    pub session_nonce: u32,
    /// Cleared when the view or tab releases this session.
    pub alive: Arc<AtomicBool>,
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
    wake: watch::Sender<u64>,
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
    wake: watch::Sender<u64>,
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
                if state.apply(key.session_nonce, &batch) {
                    for intent in state.terminal.drain_intents() {
                        handle_intent(intent, &state, |bytes| {
                            let _ = services.write_pty(&key, &bytes);
                        });
                    }
                    wake.send_modify(|generation| *generation = generation.wrapping_add(1));
                }
                flow.ack(length);
            }
        })
        .map_err(|error| format!("Failed to start the GPUI output parser: {error}"))
}
