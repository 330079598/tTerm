use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch, Mutex as TokioMutex, RwLock};

pub type PtyMap = Arc<RwLock<HashMap<String, PtySession>>>;
pub type HostPromptMap = Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>;

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
