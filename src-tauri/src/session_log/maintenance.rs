//! Background upkeep of the log directory: removing logs the retention
//! settings no longer keep, measuring how much space the directory takes,
//! and cleaning up after compression a quit cut short.

use super::{emit_status, retention, rotating, SessionLogState, CLEANUP_INTERVAL};
use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard};
use std::time::{Instant, SystemTime};
use tauri::{AppHandle, Emitter, Manager};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

impl SessionLogState {
    /// Runs upkeep on a background thread: the directory is measured every
    /// time, and old logs are removed at most once per `CLEANUP_INTERVAL`
    /// unless `force_cleanup`. Requests that come in while it runs are folded
    /// into one more run. Never takes the manager lock on the caller's
    /// thread, so it is safe to call while a log is closing.
    pub(super) fn schedule_maintenance(&self, app: &AppHandle, force_cleanup: bool) {
        if force_cleanup {
            self.force_cleanup.store(true, Ordering::Release);
        }
        if self.maintenance_running.swap(true, Ordering::AcqRel) {
            self.maintenance_again.store(true, Ordering::Release);
            return;
        }
        let app = app.clone();
        let spawned = std::thread::Builder::new()
            .name("tterm-log-maintenance".to_string())
            .spawn(move || {
                let state = app.state::<SessionLogState>();
                loop {
                    state.maintain(&app);
                    if state.maintenance_again.swap(false, Ordering::AcqRel) {
                        continue;
                    }
                    state.maintenance_running.store(false, Ordering::Release);
                    // A request that came in just before the flag dropped.
                    if state.maintenance_again.swap(false, Ordering::AcqRel)
                        && !state.maintenance_running.swap(true, Ordering::AcqRel)
                    {
                        continue;
                    }
                    break;
                }
            });
        if spawned.is_err() {
            self.maintenance_running.store(false, Ordering::Release);
        }
    }

    fn maintain(&self, app: &AppHandle) {
        let Ok((directory, retention, in_use)) = self.directory_state() else {
            return;
        };
        self.recover_once(&directory);
        let mut removed = 0;
        if !retention.is_off() && self.cleanup_due() {
            match retention::clean(&directory, retention, &in_use, SystemTime::now()) {
                Ok(report) => removed = report.removed,
                Err(message) => {
                    self.set_error(message.clone());
                    let _ = app.emit_to(
                        tauri::EventTarget::any(),
                        "terminal-log-error",
                        serde_json::json!({ "tabId": null, "message": message }),
                    );
                }
            }
        }
        let usage = directory_size(&directory);
        let previous = self.disk_usage.swap(usage, Ordering::Relaxed);
        if removed > 0 || previous != usage {
            emit_status(app, self);
        }
    }

    fn cleanup_due(&self) -> bool {
        let forced = self.force_cleanup.swap(false, Ordering::AcqRel);
        let mut last = lock(&self.last_cleanup);
        if !forced && last.is_some_and(|last| last.elapsed() < CLEANUP_INTERVAL) {
            return false;
        }
        *last = Some(Instant::now());
        true
    }

    /// Cleans up after interrupted compression, once per log directory.
    fn recover_once(&self, directory: &Path) {
        {
            let mut recovered = lock(&self.recovered_directory);
            if recovered.as_deref() == Some(directory) {
                return;
            }
            *recovered = Some(directory.to_path_buf());
        }
        rotating::recover_compression(directory);
    }
}

fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            entry
                .metadata()
                .map(|metadata| {
                    if metadata.is_dir() {
                        directory_size(&entry.path())
                    } else {
                        metadata.len()
                    }
                })
                .unwrap_or(0)
        })
        .sum()
}
