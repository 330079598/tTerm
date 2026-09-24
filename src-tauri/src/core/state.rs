use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};
use tokio::sync::{mpsc, oneshot, watch, Mutex as TokioMutex, RwLock};

pub type PtyMap = Arc<RwLock<HashMap<String, PtySession>>>;
pub type HostPromptMap = Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>;
/// A plain `std::sync::RwLock`, not `tokio::sync::RwLock`: this map is
/// touched from both a genuine OS thread (the local-PTY reader thread,
/// which can't `.await`) and async tasks (the SSH connection task, where
/// `tokio::sync::RwLock::blocking_*` would panic — "cannot block the
/// current thread from within a runtime"). Every access here is a brief,
/// never-held-across-`.await` map operation, exactly the case `std::sync`
/// primitives are for.
pub type ZmodemMap = Arc<StdRwLock<HashMap<String, ZmodemTabHandle>>>;

/// Published while a ZMODEM transfer is in flight for a tab. The actual
/// protocol engine and file I/O live entirely on whichever thread/task
/// drives that channel's bytes (the local-PTY reader thread, or the async
/// SSH connection task) — never here — so this handle stays deliberately
/// thin: just enough for `write_pty` to know to suppress keystrokes, and
/// for a cancel request from any thread to reach an in-progress transfer.
pub struct ZmodemTabHandle {
    pub session_nonce: u32,
    pub transfer_id: String,
    pub cancel_requested: Arc<AtomicBool>,
}

/// A `rz`-style (send-direction) trigger was detected but the user hasn't
/// picked local files to send yet. `zmodem_start_send` takes this out of the
/// map (there is at most one per tab) and moves it onto a dedicated OS
/// thread that drives the whole upload — because ZMODEM's send side streams
/// data without waiting for a peer ack per chunk, that thread can't just
/// react to incoming bytes the way the receive side does; it needs to keep
/// pushing chunks on its own, so it needs a thread of its own regardless of
/// whether the channel underneath is a local PTY or an SSH channel.
/// `incoming_rx` is fed by whichever thread/task owns that channel's reads
/// (the PTY reader thread, or the SSH connection task) once it recognizes
/// this tab is now piped to a send session instead of processed inline;
/// `active` is the same write-back handle `write_pty` already uses, valid
/// for both `ActiveSession::Local` and `ActiveSession::Ssh`.
pub struct ZmodemArmedSend {
    pub session_nonce: u32,
    pub incoming_rx: std::sync::mpsc::Receiver<Vec<u8>>,
    pub active: Arc<TokioMutex<Option<ActiveSession>>>,
}
pub type ZmodemArmedSendMap = Arc<StdMutex<HashMap<String, ZmodemArmedSend>>>;

/// One entry per tab for as long as the tab exists (unlike `ZmodemMap`,
/// which only holds an entry while a transfer is actually in flight):
/// lets the manual "Send/Receive Files via ZMODEM" keymap actions force
/// detection on for this tab's next `rz`/`sz` invite even when the global
/// `zmodem_auto_detect_enabled` setting is off, without needing to touch
/// the already-running reader thread/task's own state directly (the only
/// way to reach it is through shared state like this).
pub type ZmodemManualDetectMap = Arc<StdMutex<HashMap<String, Arc<AtomicBool>>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Terminal,
    Ssh,
}

pub enum ActiveSession {
    Local(crate::terminal::ActivePty),
    Ssh(ActiveSsh),
}

pub struct ActiveSsh {
    pub input_tx: mpsc::UnboundedSender<Vec<u8>>,
    pub resize_tx: mpsc::UnboundedSender<(u16, u16)>,
    pub task: tokio::task::JoinHandle<()>,
    /// Recent output of this connection attempt, for saved-password writes.
    pub output_tail: Arc<crate::terminal::OutputTail>,
}

/// Lock-free atomic storage for terminal dimensions (rows, cols).
#[derive(Debug)]
pub struct AtomicTerminalSize(AtomicU32);

impl AtomicTerminalSize {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self(AtomicU32::new(((rows as u32) << 16) | (cols as u32)))
    }

    pub fn load(&self) -> (u16, u16) {
        let packed = self.0.load(Ordering::Relaxed);
        ((packed >> 16) as u16, packed as u16)
    }

    pub fn store(&self, rows: u16, cols: u16) {
        let packed = ((rows as u32) << 16) | (cols as u32);
        self.0.store(packed, Ordering::Relaxed);
    }
}

pub struct PtySession {
    pub pid: u32,
    pub session_nonce: u32,
    pub active: Arc<TokioMutex<Option<ActiveSession>>>,
    pub stop_tx: watch::Sender<bool>,
    pub supervisor: tokio::task::JoinHandle<()>,
    /// Latest terminal dimensions, kept so an automatic reconnect can re-open
    /// the SSH channel at the current size instead of the original one.
    pub size: Arc<AtomicTerminalSize>,
}

pub enum SessionExitSignal {
    Terminated,
    Recoverable {
        reason: String,
        connected_duration: Option<std::time::Duration>,
    },
    NonRecoverable(String),
}
