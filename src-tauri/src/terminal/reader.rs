//! The thread that reads a session's output: a local shell's PTY, or a
//! serial port, into the terminal, through ZMODEM and the session log.

use crate::core::state::ChannelContext;
use crate::zmodem::session::ZmodemSettings;
use std::io::{Read, Write};
use std::sync::PoisonError;
use std::thread;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

/// Result of reading a PTY reader to end-of-stream.
#[derive(Debug, PartialEq, Eq)]
pub enum ReaderOutcome {
    Terminated,
    Failed(String),
}

/// Blocking read loop shared by the reader thread. Each read parks until the
/// next output (interactive keystroke echoes flush immediately); coalescing
/// into batches is the io_batcher's job, so a sustained flood still reaches
/// the webview as few large IPC messages.
///
/// The PTY fd is blocking: reads return data or park, never `WouldBlock`, so
/// this loop must not try to drain the fd between blocking reads — an extra
/// read would withhold the bytes already read until more output arrives.
///
/// `log_chunk` records raw bytes to the session log; `deliver` forwards one
/// batch to the webview. Split out from the thread spawn so the read behavior
/// is testable without a live Tauri app.
pub fn run_pty_reader<R, L, D>(
    mut reader: R,
    mut log_chunk: L,
    mut deliver: D,
    finish: impl FnOnce(),
) -> ReaderOutcome
where
    R: Read,
    L: FnMut(&[u8]),
    D: FnMut(&[u8]),
{
    let mut buf = [0u8; 65536];
    let mut result: Result<(), String> = Ok(());
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                log_chunk(&buf[..n]);
                deliver(&buf[..n]);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                result = Err(format!("PTY read failed: {e}"));
                break;
            }
        }
    }

    // Flush every buffered byte to the webview before the exit event so
    // trailing output (e.g. a final prompt or exit message) is not lost.
    finish();

    match result {
        Ok(()) => ReaderOutcome::Terminated,
        Err(reason) => ReaderOutcome::Failed(reason),
    }
}

/// What an [`OutputFilter`] makes of a batch of output.
pub struct FilteredOutput {
    /// What the terminal shows.
    pub display: Vec<u8>,
    /// What the session log records, when it is not `display`.
    pub log: Option<Vec<u8>>,
}

/// Turns output into what the terminal shows, for a device whose bytes are
/// not UTF-8 text as they come, such as a serial port in another charset.
/// It sees the output ZMODEM leaves, and the session log records what it
/// returns instead of the raw bytes.
pub trait OutputFilter: Send {
    fn filter(&mut self, output: Vec<u8>) -> FilteredOutput;
}

pub fn spawn_reader_thread(
    reader: Box<dyn Read + Send>,
    channel: ChannelContext,
    exit_tx: mpsc::UnboundedSender<crate::core::state::SessionExitSignal>,
    sender: Option<super::io_batcher::TerminalOutputSender>,
    settings: ZmodemSettings,
    mut filter: Option<Box<dyn OutputFilter>>,
) {
    thread::spawn(move || {
        // Shared between the deliver closure and the final flush: the sender
        // must outlive both, so route it through interior mutability.
        let sender = std::sync::Mutex::new(sender);
        let log_app = channel.app.clone();
        let log_tab_id = channel.tab_id.clone();
        // Owned solely by this thread: detection state and, once triggered,
        // the protocol engine + open destination file. Only the thin
        // `ZmodemTabHandle` published into `zmodem_map` is ever touched from
        // another thread (a manual cancel, or `write_pty` checking whether
        // to suppress keystrokes).
        let mut zmodem_driver =
            crate::zmodem::session::ZmodemReceiveDriver::new(&channel, settings);
        // Once a `rz`-style trigger arms this tab for the send direction,
        // every subsequent byte is piped here instead of through
        // `zmodem_driver` — see `ZmodemSendPipe`'s doc comment.
        let mut zmodem_send_pipe = crate::zmodem::ZmodemSendPipe::default();
        let log_raw_output = filter.is_none();

        let outcome = run_pty_reader(
            reader,
            |chunk| {
                if log_raw_output {
                    crate::session_log::record_output(&log_app, &log_tab_id, chunk);
                }
            },
            |chunk| {
                if zmodem_send_pipe.try_forward(chunk) {
                    return;
                }
                let step = zmodem_driver.process(chunk);
                if !step.outgoing.is_empty() {
                    let mut guard = channel.active.blocking_lock();
                    match guard.as_mut() {
                        Some(crate::core::state::ActiveSession::Local(local)) => {
                            let _ = local.writer.write_all(&step.outgoing);
                        }
                        Some(crate::core::state::ActiveSession::Serial(serial)) => {
                            let _ = serial.writer.write_all(&step.outgoing);
                        }
                        _ => {}
                    }
                }
                if let Some(remainder) = step.send_requested {
                    let tx = crate::zmodem::arm_send(&channel, remainder);
                    zmodem_send_pipe.arm(tx);
                    return;
                }
                if step.passthrough.is_empty() {
                    return;
                }
                let display = match filter.as_mut() {
                    Some(filter) => {
                        let filtered = filter.filter(step.passthrough);
                        let log = filtered.log.as_deref().unwrap_or(&filtered.display);
                        crate::session_log::record_output(&channel.app, &channel.tab_id, log);
                        filtered.display
                    }
                    None => step.passthrough,
                };
                if display.is_empty() {
                    return;
                }
                match sender
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .as_mut()
                {
                    Some(sender) => sender.send(display),
                    None => emit_pty_output(
                        &channel.app,
                        &channel.tab_id,
                        String::from_utf8_lossy(&display).into_owned(),
                    ),
                }
            },
            || {
                if let Some(sender) = sender
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .as_mut()
                {
                    sender.finish();
                }
            },
        );

        let _ = exit_tx.send(match outcome {
            ReaderOutcome::Terminated => crate::core::state::SessionExitSignal::Terminated,
            ReaderOutcome::Failed(reason) => crate::core::state::SessionExitSignal::Recoverable {
                reason,
                connected_duration: None,
            },
        });
    });
}

fn emit_pty_output(app: &AppHandle, tab_id: &str, payload: String) {
    let event_name = format!("pty-output-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
}
