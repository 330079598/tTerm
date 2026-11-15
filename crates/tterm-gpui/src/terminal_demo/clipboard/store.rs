//! A bounded hand-off for OSC 52 writes from the parser to the window.
//!
//! The parser never touches a platform clipboard. It replaces the single
//! pending write and wakes the UI, which applies the latest one. A store
//! larger than the bound is dropped rather than held in memory.

use std::sync::Mutex;

use tokio::sync::watch;

/// The most bytes one OSC 52 store may ask to put on the clipboard.
pub(crate) const MAX_CLIPBOARD_STORE_BYTES: usize = 1024 * 1024;

/// The latest OSC 52 write and the signal that wakes the window for it.
pub(crate) struct ClipboardWrites {
    enabled: bool,
    pending: Mutex<Option<String>>,
    changed: watch::Sender<u64>,
}

impl ClipboardWrites {
    /// Creates the slot for one session generation.
    pub(crate) fn new(enabled: bool) -> Self {
        let (changed, _receiver) = watch::channel(0_u64);
        Self {
            enabled,
            pending: Mutex::new(None),
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
        self.enabled
    }

    /// Replaces the pending write with `text` and wakes the window.
    ///
    /// Even a hidden tab keeps the pending write, because the clipboard is a
    /// user-visible effect independent of drawing. `false` means the store
    /// was rejected by the setting, the byte bound or an empty payload.
    pub(crate) fn store(&self, text: String) -> bool {
        if !self.enabled || text.is_empty() || text.len() > MAX_CLIPBOARD_STORE_BYTES {
            return false;
        }
        {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *pending = Some(text);
        }
        self.changed
            .send_modify(|generation| *generation = generation.wrapping_add(1));
        true
    }

    /// Takes the latest pending write, if the window has not consumed it.
    pub(crate) fn take(&self) -> Option<String> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    /// Subscribes to wake-ups. The watch channel holds only the latest
    /// generation, so many stores coalesce into one UI wake-up.
    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }
}
