use std::collections::HashMap;
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

pub struct PtySession {
    pub active: Arc<TokioMutex<Option<ActiveSession>>>,
    pub stop_tx: watch::Sender<bool>,
    pub supervisor: tokio::task::JoinHandle<()>,
}

pub enum SessionExitSignal {
    Terminated,
    Recoverable(String),
    NonRecoverable(String),
}
