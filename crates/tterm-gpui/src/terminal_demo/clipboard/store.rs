//! A bounded hand-off for OSC 52 writes from the parser to the window.
//!
//! The parser never touches a platform clipboard. It replaces the single
//! pending write and wakes the UI, which applies the latest one. A store
//! larger than the bound is dropped rather than held in memory.
//!
//! The setting and the pending write share one lock. Turning writes off
//! therefore clears a store that has not been applied yet, and a store that
//! races with the change either lands before the revoke or is rejected after
//! it; a later enable can never replay the revoked text.

use std::sync::{Mutex, MutexGuard, PoisonError};

use tokio::sync::watch;

/// The most bytes one OSC 52 store may ask to put on the clipboard.
pub(crate) const MAX_CLIPBOARD_STORE_BYTES: usize = 1024 * 1024;

/// The setting and the one write that has not reached the clipboard.
struct Slot {
    epoch: u64,
    enabled: bool,
    pending: Option<String>,
}

/// The latest OSC 52 write and the signal that wakes the window for it.
pub(crate) struct ClipboardWrites {
    slot: Mutex<Slot>,
    changed: watch::Sender<u64>,
}

impl ClipboardWrites {
    /// Creates the slot for one session generation.
    pub(crate) fn new(enabled: bool) -> Self {
        let (changed, _receiver) = watch::channel(0_u64);
        Self {
            slot: Mutex::new(Slot {
                epoch: 0,
                enabled,
                pending: None,
            }),
            changed,
        }
    }

    /// A slot for tests and sessions that may not write the clipboard.
    #[cfg(test)]
    pub(crate) fn disabled() -> Self {
        Self::new(false)
    }

    /// Whether this session may write the clipboard at all.
    pub(crate) fn enabled(&self) -> bool {
        self.slot().enabled
    }

    /// Applies a committed setting change to this generation.
    ///
    /// Disabling revokes a pending write in the same critical section that
    /// rejects later stores, so rapid off/on cannot deliver text that was
    /// queued before the change. Enabling leaves no old write to replay.
    pub(crate) fn set_enabled(&self, enabled: bool) {
        let mut slot = self.slot();
        if slot.enabled == enabled {
            return;
        }
        slot.epoch = slot.epoch.wrapping_add(1);
        slot.enabled = enabled;
        if !enabled {
            slot.pending = None;
        }
    }

    /// Replaces the pending write with `text` and wakes the window.
    ///
    /// Even a hidden tab keeps the pending write, because the clipboard is a
    /// user-visible effect independent of drawing. `false` means the store
    /// was rejected by the setting, the byte bound or an empty payload.
    #[cfg(test)]
    pub(crate) fn store(&self, text: String) -> bool {
        self.store_at(self.epoch(), text)
    }

    /// Identifies the permission in effect when parsing begins, before intents are drained.
    pub(crate) fn epoch(&self) -> u64 {
        self.slot().epoch
    }

    pub(crate) fn store_at(&self, epoch: u64, text: String) -> bool {
        if text.is_empty() || text.len() > MAX_CLIPBOARD_STORE_BYTES {
            return false;
        }
        {
            let mut slot = self.slot();
            if !slot.enabled || slot.epoch != epoch {
                return false;
            }
            slot.pending = Some(text);
        }
        self.changed
            .send_modify(|generation| *generation = generation.wrapping_add(1));
        true
    }

    /// Takes the latest pending write, if the window has not consumed it.
    pub(crate) fn take(&self) -> Option<String> {
        let mut slot = self.slot();
        if !slot.enabled {
            slot.pending = None;
            return None;
        }
        slot.pending.take()
    }

    /// Subscribes to wake-ups. The watch channel holds only the latest
    /// generation, so many stores coalesce into one UI wake-up.
    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }

    fn slot(&self) -> MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
