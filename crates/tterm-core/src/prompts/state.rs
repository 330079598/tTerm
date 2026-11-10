use super::{OwnerState, PromptId, PromptKind, PromptOutcome, PromptRequest};
use crate::events::EventSink;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use tokio::sync::oneshot;

pub(super) type Key = (PromptKind, PromptId);

pub(super) struct Waiter {
    pub owner: Arc<OwnerState>,
    pub sender: oneshot::Sender<PromptOutcome>,
}

pub(super) struct Entry {
    pub request: PromptRequest,
    pub waiters: HashMap<u64, Waiter>,
    pub resolving: bool,
}

#[derive(Default)]
pub(super) struct Pending {
    pub sequence: u64,
    pub shutdown: bool,
    pub owners: HashMap<u64, Weak<OwnerState>>,
    pub active: HashMap<Key, u64>,
    pub entries: HashMap<u64, Entry>,
}

impl Pending {
    pub fn next_id(&mut self) -> Result<u64, String> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Prompt sequence exhausted; restart the application")?;
        Ok(self.sequence)
    }

    pub fn take(&mut self, generation: u64) -> Option<Entry> {
        let entry = self.entries.remove(&generation)?;
        let key = (entry.request.kind, entry.request.id.clone());
        if self.active.get(&key) == Some(&generation) {
            self.active.remove(&key);
        }
        Some(entry)
    }
}

pub(super) struct Shared {
    pub pending: Mutex<Pending>,
    pub events: Arc<dyn EventSink>,
}

impl Shared {
    pub fn lock(&self) -> MutexGuard<'_, Pending> {
        self.pending
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    pub fn remove_waiter(&self, generation: u64, waiter: u64) {
        let removed = {
            let mut state = self.lock();
            let removed = state
                .entries
                .get_mut(&generation)
                .and_then(|entry| entry.waiters.remove(&waiter));
            if state
                .entries
                .get(&generation)
                .is_some_and(|entry| entry.waiters.is_empty())
            {
                state.take(generation);
            }
            removed
        };
        drop(removed);
    }
}

pub(super) fn expired_error(kind: PromptKind) -> String {
    match kind {
        PromptKind::HostKey => "Host key prompt expired",
        PromptKind::Auth => "Authentication prompt expired",
        PromptKind::ProxyCommand => "The proxy command question has expired.",
    }
    .to_string()
}
