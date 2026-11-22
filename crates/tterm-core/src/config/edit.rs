//! Serializes read-modify-write edits to the shared config file.
//!
//! The atomic writer protects a single replacement, but a setting that reads
//! the file, changes one field and writes it back needs a wider lock. Without
//! one, a template save and a scrollback save can both start from the same
//! bytes and the later write silently restores the other setting's old value.

use std::sync::{Mutex, MutexGuard, OnceLock};

static CONFIG_EDIT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Holds the process-wide lock for one config read-modify-write operation.
pub(super) fn lock() -> Result<MutexGuard<'static, ()>, String> {
    CONFIG_EDIT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "The config edit lock was poisoned".to_string())
}
