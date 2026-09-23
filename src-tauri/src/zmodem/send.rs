//! Drives the ZMODEM send (upload) direction from the moment `rz`'s trigger
//! is detected until the session ends. Runs on a dedicated OS thread owned
//! by the `zmodem_start_send` Tauri command: unlike the receive side, this
//! can't just react to incoming bytes — ZMODEM's send direction streams
//! subpacket after subpacket without waiting for a peer ack per chunk (see
//! `frame.rs`'s doc comment on real `sz`'s streaming style), so something
//! has to actively keep pushing them regardless of whether new bytes have
//! arrived. Write-back goes through the same `ActiveSession` handle
//! `write_pty` already uses (works for both a local PTY and an SSH channel,
//! from any plain OS thread), and progress events reuse `events.rs`'s
//! shapes so the frontend treats both directions identically.
//!
//! The reader thread/task that owns the channel's bytes forwards them here
//! via `incoming` instead of processing them itself once armed (see
//! `ZmodemArmedSend`); when this function returns, `incoming` (and the
//! `Receiver` inside it) is dropped, so the reader's next `send()` on the
//! paired `Sender` fails — that failure is the signal it uses to fall back
//! to normal processing without any extra coordination.

use std::fs::File;
use std::io::{Read as _, Seek, SeekFrom, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex as TokioMutex;

use super::events::{
    self, EmitFn, TransferCompletePayload, TransferProgressPayload, TransferStartPayload,
};
use super::protocol::{ZmodemAction, ZmodemDirection, ZmodemEngine};
use crate::core::state::ActiveSession;
use crate::core::ZmodemMap;

/// Forces a local PTY into raw mode for the duration of a send session, and
/// restores the original settings when the returned guard drops. Binary
/// ZMODEM frames must not pass through cooked-mode line discipline (echo,
/// XON/XOFF flow control, CR/NL translation) — a real local-PTY test
/// against real `rz` showed this corrupting frames badly enough to draw a
/// `ZNAK` on essentially every attempt, since `rz` does not reliably switch
/// the pty to raw mode itself before our very next write can land. Only
/// applies to a local PTY: over SSH, the pty (if any) is on the remote host
/// and out of our control, which is what the engine's own `ZNAK`/`ZRINIT`
/// retry handling (see `last_zfile`) exists to cover instead.
#[cfg(unix)]
mod raw_mode {
    use std::os::unix::io::RawFd;
    use std::sync::Arc;

    use tokio::sync::Mutex as TokioMutex;

    use crate::core::state::ActiveSession;

    pub struct RawModeGuard {
        fd: RawFd,
        original: libc::termios,
    }

    impl RawModeGuard {
        fn engage(fd: RawFd) -> Option<Self> {
            unsafe {
                let mut original: libc::termios = std::mem::zeroed();
                if libc::tcgetattr(fd, &mut original) != 0 {
                    return None;
                }
                let mut raw = original;
                libc::cfmakeraw(&mut raw);
                if libc::tcsetattr(fd, libc::TCSANOW, &raw) != 0 {
                    return None;
                }
                Some(Self { fd, original })
            }
        }
    }

    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = libc::tcsetattr(self.fd, libc::TCSANOW, &self.original);
            }
        }
    }

    pub fn engage_for_local_pty(
        active: &Arc<TokioMutex<Option<ActiveSession>>>,
    ) -> Option<RawModeGuard> {
        let guard = active.blocking_lock();
        match guard.as_ref() {
            Some(ActiveSession::Local(local)) => RawModeGuard::engage(local.master.as_raw_fd()?),
            _ => None,
        }
    }
}

#[cfg(not(unix))]
mod raw_mode {
    use std::sync::Arc;

    use tokio::sync::Mutex as TokioMutex;

    use crate::core::state::ActiveSession;

    pub struct RawModeGuard;

    pub fn engage_for_local_pty(
        _active: &Arc<TokioMutex<Option<ActiveSession>>>,
    ) -> Option<RawModeGuard> {
        None
    }
}

/// ZMODEM's traditional subpacket size; matches what real `sz` uses.
const CHUNK_SIZE: usize = 1024;
/// Safety net only: real `rz`/`sz` give up much sooner on their own if the
/// other side goes silent.
const FEED_TIMEOUT: Duration = Duration::from_secs(30);
const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(120);

pub struct ZmodemSendParams {
    pub emit: EmitFn,
    pub tab_id: String,
    pub zmodem_map: ZmodemMap,
    pub incoming: Receiver<Vec<u8>>,
    pub active: Arc<TokioMutex<Option<ActiveSession>>>,
    pub cancel_requested: Arc<AtomicBool>,
    pub paths: Vec<PathBuf>,
}

/// Runs the whole send session to completion, cancellation, or a peer
/// timeout/abort. Blocking — call this from a dedicated thread.
pub fn run(params: ZmodemSendParams) {
    let ZmodemSendParams {
        emit,
        tab_id,
        zmodem_map,
        incoming,
        active,
        cancel_requested,
        paths,
    } = params;

    // Held for the whole session; restores the original termios on drop
    // (every exit path below, including early returns).
    let _raw_mode_guard = raw_mode::engage_for_local_pty(&active);

    let mut engine = ZmodemEngine::new(ZmodemDirection::Send);

    // Drain the trigger bytes that armed this session (the very first
    // message the reader thread ever forwarded here) before anything else,
    // so later reads from `incoming` stay correctly ordered.
    if let Ok(chunk) = incoming.recv_timeout(FEED_TIMEOUT) {
        let result = engine.feed(&chunk);
        write_back(&active, &result.outgoing);
    }

    for path in &paths {
        if cancel_requested.load(Ordering::Relaxed) {
            break;
        }
        if !send_one_file(
            &mut engine,
            &incoming,
            &active,
            &cancel_requested,
            &emit,
            &zmodem_map,
            &tab_id,
            path,
        ) {
            zmodem_map.write().unwrap().remove(&tab_id);
            return;
        }
    }

    // No more files: wait for the peer's final "ready" signal, send ZFIN,
    // and give it one chance to echo back before closing out either way.
    if !cancel_requested.load(Ordering::Relaxed) {
        if let Some(fin) =
            feed_until_ready(&mut engine, &incoming, &active, &cancel_requested, |e| {
                e.finish_send()
            })
        {
            write_back(&active, &fin);
            if let Ok(chunk) = incoming.recv_timeout(FEED_TIMEOUT) {
                let result = engine.feed(&chunk);
                write_back(&active, &result.outgoing);
            }
        }
    }

    zmodem_map.write().unwrap().remove(&tab_id);
}

/// Sends one file: waits for the peer to be ready for it, announces it,
/// waits for the peer's starting position, then streams it. Returns `false`
/// if the whole session should stop (cancelled, peer aborted/timed out).
#[allow(clippy::too_many_arguments)]
fn send_one_file(
    engine: &mut ZmodemEngine,
    incoming: &Receiver<Vec<u8>>,
    active: &Arc<TokioMutex<Option<ActiveSession>>>,
    cancel_requested: &AtomicBool,
    emit: &EmitFn,
    zmodem_map: &ZmodemMap,
    tab_id: &str,
    path: &Path,
) -> bool {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("ZMODEM: failed to open {}: {err}", path.display());
            return true; // Skip this file, try the next one in the batch.
        }
    };
    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);

    let Some(kickoff) = feed_until_ready(engine, incoming, active, cancel_requested, |e| {
        e.begin_send(&file_name, file_size)
    }) else {
        return false;
    };
    write_back(active, &kickoff);

    let transfer_id = events::new_transfer_id();
    // A multi-file batch reuses the same tab-level handle (and its
    // `cancel_requested`) across files, only rotating the displayed
    // transfer id — mirrors the receive driver's `on_file_complete`.
    if let Some(handle) = zmodem_map.write().unwrap().get_mut(tab_id) {
        handle.transfer_id = transfer_id.clone();
    }
    events::emit(
        emit,
        "transfer-start",
        TransferStartPayload {
            transfer_id: transfer_id.clone(),
            direction: "upload",
            file_name,
            file_size,
            local_path: path.display().to_string(),
        },
    );

    let Some(offset) = feed_until_pos(engine, incoming, active, cancel_requested) else {
        events::emit(
            emit,
            "transfer-complete",
            TransferCompletePayload {
                transfer_id,
                success: false,
                cancelled: cancel_requested.load(Ordering::Relaxed),
                error: None,
            },
        );
        return false;
    };

    match push_file(
        engine,
        active,
        &mut file,
        offset,
        file_size,
        &transfer_id,
        emit,
        cancel_requested,
    ) {
        Some(_) => {
            events::emit(
                emit,
                "transfer-complete",
                TransferCompletePayload {
                    transfer_id,
                    success: true,
                    cancelled: false,
                    error: None,
                },
            );
            true
        }
        None => {
            events::emit(
                emit,
                "transfer-complete",
                TransferCompletePayload {
                    transfer_id,
                    success: false,
                    cancelled: true,
                    error: None,
                },
            );
            false
        }
    }
}

/// Streams `file` from `offset` to `file_size`, calling `send_chunk` for
/// each subpacket without waiting for a peer ack between them (matching
/// real `sz`'s streaming style — our transport is a reliable, ordered
/// stream, so mid-file corruption requiring a resend isn't expected; a
/// resend request is still honored at the *start* of each file via
/// `feed_until_pos`). Returns the final offset sent, or `None` if cancelled.
#[allow(clippy::too_many_arguments)]
fn push_file(
    engine: &mut ZmodemEngine,
    active: &Arc<TokioMutex<Option<ActiveSession>>>,
    file: &mut File,
    mut offset: u64,
    file_size: u64,
    transfer_id: &str,
    emit: &EmitFn,
    cancel_requested: &AtomicBool,
) -> Option<u64> {
    let _ = file.seek(SeekFrom::Start(offset));
    let started_at = Instant::now();
    let mut last_progress_at = started_at;
    let mut buf = vec![0u8; CHUNK_SIZE];

    loop {
        if cancel_requested.load(Ordering::Relaxed) {
            return None;
        }
        let n = file.read(&mut buf).unwrap_or(0);
        let is_last = n == 0 || offset + n as u64 >= file_size;
        if let Some(bytes) = engine.send_chunk(offset, &buf[..n], is_last) {
            write_back(active, &bytes);
        }
        offset += n as u64;

        let now = Instant::now();
        if is_last || now.duration_since(last_progress_at) >= PROGRESS_EMIT_INTERVAL {
            last_progress_at = now;
            let elapsed = now.duration_since(started_at).as_secs_f64().max(0.001);
            let speed = (offset as f64 / elapsed) as u64;
            events::emit(
                emit,
                "transfer-progress",
                TransferProgressPayload {
                    transfer_id: transfer_id.to_string(),
                    transferred: offset,
                    speed,
                },
            );
        }
        if is_last {
            return Some(offset);
        }
    }
}

/// Feeds `incoming` messages into `engine` until `attempt` returns a
/// non-empty result (the engine reached the state `attempt` needs) or the
/// session ends/is cancelled/times out.
fn feed_until_ready(
    engine: &mut ZmodemEngine,
    incoming: &Receiver<Vec<u8>>,
    active: &Arc<TokioMutex<Option<ActiveSession>>>,
    cancel_requested: &AtomicBool,
    mut attempt: impl FnMut(&mut ZmodemEngine) -> Vec<u8>,
) -> Option<Vec<u8>> {
    loop {
        let bytes = attempt(engine);
        if !bytes.is_empty() {
            return Some(bytes);
        }
        if cancel_requested.load(Ordering::Relaxed) {
            return None;
        }
        match incoming.recv_timeout(FEED_TIMEOUT) {
            Ok(chunk) => {
                let result = engine.feed(&chunk);
                write_back(active, &result.outgoing);
                if result
                    .actions
                    .iter()
                    .any(|a| matches!(a, ZmodemAction::SessionEnded | ZmodemAction::PeerCancelled))
                {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
}

/// Feeds `incoming` messages until the peer tells us where to start (a
/// `ZRPOS`, surfaced as `ZmodemAction::ResendFrom`) or the session
/// ends/is cancelled/times out.
fn feed_until_pos(
    engine: &mut ZmodemEngine,
    incoming: &Receiver<Vec<u8>>,
    active: &Arc<TokioMutex<Option<ActiveSession>>>,
    cancel_requested: &AtomicBool,
) -> Option<u64> {
    loop {
        if cancel_requested.load(Ordering::Relaxed) {
            return None;
        }
        match incoming.recv_timeout(FEED_TIMEOUT) {
            Ok(chunk) => {
                let result = engine.feed(&chunk);
                write_back(active, &result.outgoing);
                for action in &result.actions {
                    match action {
                        ZmodemAction::ResendFrom(offset) => return Some(*offset),
                        ZmodemAction::SessionEnded | ZmodemAction::PeerCancelled => return None,
                        _ => {}
                    }
                }
            }
            Err(_) => return None,
        }
    }
}

fn write_back(active: &Arc<TokioMutex<Option<ActiveSession>>>, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let mut guard = active.blocking_lock();
    match guard.as_mut() {
        Some(ActiveSession::Local(local)) => {
            let _ = local.writer.write_all(bytes);
        }
        Some(ActiveSession::Ssh(ssh)) => {
            let _ = ssh.input_tx.send(bytes.to_vec());
        }
        None => {}
    }
}
