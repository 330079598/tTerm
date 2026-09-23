//! ZMODEM (rz/sz) file-transfer protocol support.
//!
//! `detect` and `protocol` are pure and I/O-free so the same code can run
//! identically inside the local-PTY reader thread and the async SSH
//! channel task; each call site stays responsible for actually writing the
//! bytes these produce back to its own channel.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use tauri::Emitter;
use tokio::sync::Mutex as TokioMutex;

use crate::core::state::ActiveSession;
use crate::core::{ZmodemArmedSend, ZmodemArmedSendMap, ZmodemMap, ZmodemTabHandle};

pub mod commands;
pub mod detect;
pub mod events;
pub mod protocol;
pub mod send;
pub mod session;

#[cfg(test)]
mod real_interop_tests;
#[cfg(test)]
mod real_pty_interop_tests;
#[cfg(test)]
mod real_pty_send_interop_tests;
#[cfg(test)]
mod real_ssh_interop_tests;
#[cfg(test)]
mod real_ssh_send_interop_tests;

/// Held locally by whichever thread/task owns a channel's bytes. `None`
/// while ZMODEM is idle or driving a receive; `Some(tx)` once a send
/// session has been armed (see `arm_send`/`ZmodemArmedSend`) and raw bytes
/// should be forwarded to it verbatim instead of processed inline.
#[derive(Default)]
pub struct ZmodemSendPipe(Option<Sender<Vec<u8>>>);

impl ZmodemSendPipe {
    pub fn arm(&mut self, tx: Sender<Vec<u8>>) {
        self.0 = Some(tx);
    }

    /// Tries to forward `chunk` if armed. Returns `true` if it was (the
    /// caller should do nothing else with these bytes this call); `false`
    /// if not armed, or the send session just ended — a failed `send()`
    /// means the send thread finished and dropped its `Receiver`, which
    /// this treats as "done" and clears itself, so the caller falls back to
    /// processing `chunk` normally with no other coordination needed.
    pub fn try_forward(&mut self, chunk: &[u8]) -> bool {
        let Some(tx) = &self.0 else {
            return false;
        };
        if tx.send(chunk.to_vec()).is_ok() {
            true
        } else {
            self.0 = None;
            false
        }
    }
}

/// Arms `tab_id` for the ZMODEM send direction once `rz`'s trigger is
/// detected: registers the bookkeeping `zmodem_start_send` and `write_pty`
/// need, notifies the frontend to prompt for local files, and returns the
/// `Sender` half the caller should keep forwarding this tab's raw bytes to
/// via `ZmodemSendPipe` from now on.
pub fn arm_send(
    app: &tauri::AppHandle,
    tab_id: &str,
    session_nonce: u32,
    zmodem_map: &ZmodemMap,
    armed_send_map: &ZmodemArmedSendMap,
    active: &Arc<TokioMutex<Option<ActiveSession>>>,
    trigger_bytes: Vec<u8>,
) -> Sender<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let cancel_requested = Arc::new(AtomicBool::new(false));
    zmodem_map.write().unwrap().insert(
        tab_id.to_string(),
        ZmodemTabHandle {
            session_nonce,
            // No file offered yet; `send.rs` sets this once one is.
            transfer_id: String::new(),
            cancel_requested,
        },
    );
    armed_send_map.lock().unwrap().insert(
        tab_id.to_string(),
        ZmodemArmedSend {
            session_nonce,
            incoming_rx: rx,
            active: active.clone(),
        },
    );
    let _ = tx.send(trigger_bytes);
    let event = format!("zmodem-send-requested-{tab_id}");
    let _ = app.emit_to(tauri::EventTarget::any(), &event, ());
    tx
}
