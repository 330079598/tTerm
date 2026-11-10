//! One-shot prompt lifetimes independent of a client or authentication protocol.
//!
//! P1-05 keeps wire payloads in their event domains. Requests carry typed events
//! so registering, cancelling and testing the broker do not depend on Tauri.

mod pending;
mod proxy;
mod state;

use crate::events::AppEvent;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::oneshot;

/// The wire request ID, interpreted within a prompt kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PromptId(pub String);

/// Distinct reply namespaces prevent a command from consuming another kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptKind {
    HostKey,
    Auth,
    ProxyCommand,
}

/// The terminal generation that owns an operation, when one exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionKey {
    pub tab_id: String,
    pub session_nonce: u32,
}

/// An operation/connection attempt minted by a broker, never a tab-name alias.
#[derive(Clone)]
pub struct PromptOwner {
    inner: Arc<OwnerState>,
}

struct OwnerState {
    id: u64,
    session: Option<SessionKey>,
    closed: AtomicBool,
    broker: std::sync::Weak<state::Shared>,
}

impl PromptOwner {
    /// Allows callers to stop multi-step authentication after scope cancellation.
    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::SeqCst)
    }
}

/// A domain-created request; only proxy requests share an existing wire ID.
#[derive(Clone)]
pub struct PromptRequest {
    pub id: PromptId,
    pub kind: PromptKind,
    pub event: AppEvent,
    /// Emitted only when a waiting request times out, never on owner cancellation.
    pub expired_event: Option<AppEvent>,
}

/// Each waiter starts its own timeout when `wait` is first polled.
pub struct PromptOptions {
    pub owner: PromptOwner,
    pub timeout: Duration,
}

/// Replies remain typed even when their transport is a command with a string ID.
#[derive(Debug, PartialEq, Eq)]
pub enum PromptReply {
    HostKey(bool),
    Auth(Option<Vec<String>>),
    ProxyCommand(bool),
}

/// User cancellation (`Auth(None)`) is separate from a closed operation.
#[derive(Debug, PartialEq, Eq)]
pub enum PromptOutcome {
    Answered(PromptReply),
    TimedOut,
    Cancelled,
    ReceiverClosed,
}

/// A single receiver with synchronous drop cleanup, including before first poll.
pub struct PromptWait {
    shared: Arc<state::Shared>,
    generation: u64,
    waiter: u64,
    id: PromptId,
    receiver: Option<oneshot::Receiver<PromptOutcome>>,
    timeout: Duration,
}

/// Holds pending senders and publishes requests through the existing EventSink.
#[derive(Clone)]
pub struct PromptBroker {
    shared: Arc<state::Shared>,
}

#[cfg(test)]
mod tests;
