//! The sessions being logged and the settings they are logged with.

use super::retention::Retention;
use super::rotating;
use super::worker::{self, LogHandle};
use super::writer::{SessionWriter, Stamp};
use super::{
    emit_error, emit_start_error, emit_status, LogConfig, SessionLogPolicy, SessionLogState,
    SessionMetadata, TerminalLogStatus,
};
use crate::config::AppConfig;
use crate::core::session::types::SessionPlan;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::MutexGuard;
use std::time::Duration;
use tauri::AppHandle;

/// How long quitting waits for logs to write out what they still hold.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(3);

struct SessionEntry {
    metadata: SessionMetadata,
    policy: SessionLogPolicy,
    rows: u16,
    cols: u16,
    writer: Option<LogHandle>,
}

#[derive(Default)]
pub(super) struct LogManager {
    pub(super) config: Option<LogConfig>,
    sessions: HashMap<String, SessionEntry>,
    /// Tabs the user started or stopped logging for by hand; the choice
    /// outlasts a reconnect.
    manual: HashMap<String, bool>,
    /// The logs each tab has written since tTerm started, oldest first. Tab
    /// ids are reused across runs, so this is what ties a tab to its logs.
    pub(super) logged: HashMap<String, Vec<String>>,
    last_error: Option<String>,
}

impl LogManager {
    fn records(&self, tab_id: &str, policy: SessionLogPolicy) -> bool {
        self.config
            .as_ref()
            .is_some_and(|config| config.records(policy, self.manual.get(tab_id).copied()))
    }

    fn has_writers(&self) -> bool {
        self.sessions.values().any(|entry| entry.writer.is_some())
    }

    fn writer(&self, tab_id: &str) -> Option<&LogHandle> {
        self.sessions
            .get(tab_id)
            .and_then(|entry| entry.writer.as_ref())
    }

    /// Starts a log for a tab's session and remembers it as the tab's.
    fn start_writer(
        &mut self,
        app: &AppHandle,
        tab_id: &str,
        entry: &SessionEntry,
        event: &str,
    ) -> Result<LogHandle, String> {
        let config = self
            .config
            .as_ref()
            .ok_or("Terminal logging is not configured")?;
        let mut writer =
            SessionWriter::create(app, config, entry.metadata.clone(), entry.rows, entry.cols)?;
        // The size lets a replay start at the size the session had.
        writer.record_event(
            event,
            serde_json::json!({ "rows": entry.rows, "cols": entry.cols }),
            Stamp::now(),
        )?;
        let handle = LogHandle::spawn(app, tab_id, writer)?;
        self.logged
            .entry(tab_id.to_string())
            .or_default()
            .push(handle.id.clone());
        Ok(handle)
    }

    /// Starts the log of a tracked session that has none.
    fn start_tracked_writer(
        &mut self,
        app: &AppHandle,
        tab_id: &str,
        event: &str,
    ) -> Result<(), String> {
        let Some(entry) = self.sessions.remove(tab_id) else {
            return Ok(());
        };
        let result = self.start_writer(app, tab_id, &entry, event);
        let (writer, result) = match result {
            Ok(writer) => (Some(writer), Ok(())),
            Err(err) => (None, Err(err)),
        };
        self.sessions
            .insert(tab_id.to_string(), SessionEntry { writer, ..entry });
        result
    }
}

impl SessionLogState {
    pub(super) fn set_error(&self, message: String) {
        if let Ok(mut manager) = self.manager.lock() {
            manager.last_error = Some(message);
        }
    }

    pub(super) fn lock_manager(&self) -> Result<MutexGuard<'_, LogManager>, String> {
        self.manager
            .lock()
            .map_err(|_| "Terminal log state is unavailable".to_string())
    }

    fn sync_active(&self, manager: &LogManager) {
        self.active.store(manager.has_writers(), Ordering::Relaxed);
    }

    pub fn validate_config(&self, config: &AppConfig) -> Result<(), String> {
        let config = LogConfig::from_app_config(config)?;
        if config.record_by_default {
            validate_directory(&config.files.directory)?;
        }
        Ok(())
    }

    pub fn apply_config(&self, app: &AppHandle, app_config: &AppConfig) -> Result<(), String> {
        let config = LogConfig::from_app_config(app_config)?;
        if config.record_by_default {
            validate_directory(&config.files.directory)?;
        }

        let mut manager = self.lock_manager()?;
        if manager.config.as_ref() == Some(&config) {
            return Ok(());
        }
        let previous = manager.config.replace(config.clone());
        let files_changed =
            previous.as_ref().map(|previous| &previous.files) != Some(&config.files);
        let retention_changed =
            previous.as_ref().map(|previous| previous.retention) != Some(config.retention);

        let mut error = None;
        let tab_ids: Vec<String> = manager.sessions.keys().cloned().collect();
        for tab_id in tab_ids {
            let policy = manager.sessions[&tab_id].policy;
            let records = manager.records(&tab_id, policy);
            let entry = manager.sessions.get_mut(&tab_id).expect("listed above");
            if let Some(writer) = entry.writer.take() {
                if records && !files_changed {
                    entry.writer = Some(writer);
                } else if records {
                    writer.close("logging_reconfigured");
                } else {
                    writer.close("logging_disabled");
                }
            }
            if records && manager.sessions[&tab_id].writer.is_none() {
                if let Err(err) = manager.start_tracked_writer(app, &tab_id, "logging_enabled") {
                    error = Some(err);
                }
            }
        }
        manager.last_error = error.clone();
        self.sync_active(&manager);
        drop(manager);
        emit_status(app, self);
        if files_changed || retention_changed {
            self.schedule_maintenance(app, true);
        }
        error.map_or(Ok(()), Err)
    }

    /// Tracks a new session and starts its log if it is logged. A log that
    /// cannot start is reported but never stops the session from connecting.
    pub(super) fn start_session(
        &self,
        app: &AppHandle,
        tab_id: &str,
        session_nonce: u32,
        plan: &SessionPlan,
        rows: u16,
        cols: u16,
    ) {
        let entry = SessionEntry {
            metadata: SessionMetadata::new(tab_id, session_nonce, plan),
            policy: plan.session_log,
            rows,
            cols,
            writer: None,
        };
        let mut manager = match self.lock_manager() {
            Ok(manager) => manager,
            Err(err) => {
                emit_start_error(app, tab_id, &err);
                return;
            }
        };
        if let Some(writer) = manager
            .sessions
            .remove(tab_id)
            .and_then(|previous| previous.writer)
        {
            writer.close("session_end");
        }
        manager.sessions.insert(tab_id.to_string(), entry);
        let failure = if manager.records(tab_id, plan.session_log) {
            manager
                .start_tracked_writer(app, tab_id, "session_start")
                .err()
        } else {
            None
        };
        if let Some(err) = &failure {
            manager.last_error = Some(err.clone());
        }
        self.sync_active(&manager);
        drop(manager);
        if let Some(err) = failure {
            emit_start_error(app, tab_id, &err);
        }
        emit_status(app, self);
    }

    /// The manager, unless nothing is being logged; output on every tab
    /// passes through here, so that case must stay cheap.
    fn manager_if_active(
        &self,
        app: &AppHandle,
        tab_id: &str,
    ) -> Option<MutexGuard<'_, LogManager>> {
        if !self.active.load(Ordering::Relaxed) {
            return None;
        }
        match self.manager.lock() {
            Ok(manager) => Some(manager),
            Err(_) => {
                emit_error(app, tab_id, "Terminal log state is unavailable");
                None
            }
        }
    }

    /// Queues output or input for the tab's log. Never waits on the disk.
    pub(super) fn record_bytes(
        &self,
        app: &AppHandle,
        tab_id: &str,
        direction: &'static str,
        data: &[u8],
    ) {
        let Some(manager) = self.manager_if_active(app, tab_id) else {
            return;
        };
        // Taken under the lock, so what reaches a log in order is timed in
        // order, whichever thread it came from.
        let at = Stamp::now();
        if direction == "input"
            && !manager
                .config
                .as_ref()
                .is_some_and(|config| config.record_input)
        {
            return;
        }
        if let Some(writer) = manager.writer(tab_id) {
            writer.data(direction, data, at);
        }
    }

    pub(super) fn record_event(
        &self,
        app: &AppHandle,
        tab_id: &str,
        event: &'static str,
        detail: serde_json::Value,
    ) {
        let Some(manager) = self.manager_if_active(app, tab_id) else {
            return;
        };
        if let Some(writer) = manager.writer(tab_id) {
            writer.event(event, detail, Stamp::now());
        }
    }

    pub(super) fn resize(&self, tab_id: &str, rows: u16, cols: u16) {
        let Ok(mut manager) = self.manager.lock() else {
            return;
        };
        let at = Stamp::now();
        if let Some(entry) = manager.sessions.get_mut(tab_id) {
            entry.rows = rows;
            entry.cols = cols;
            if let Some(writer) = &entry.writer {
                writer.resize(rows, cols, at);
            }
        }
    }

    pub(super) fn set_recording(
        &self,
        app: &AppHandle,
        tab_id: &str,
        recording: bool,
    ) -> Result<TerminalLogStatus, String> {
        let mut manager = self.lock_manager()?;
        if manager.config.is_none() {
            return Err("Terminal logging is not configured".to_string());
        }
        let previous = manager.manual.insert(tab_id.to_string(), recording);
        let mut result = Ok(());
        let has_writer = manager
            .sessions
            .get(tab_id)
            .map(|entry| entry.writer.is_some());
        match (recording, has_writer) {
            (true, Some(false)) => {
                result = manager.start_tracked_writer(app, tab_id, "logging_started");
            }
            (false, Some(true)) => {
                if let Some(writer) = manager
                    .sessions
                    .get_mut(tab_id)
                    .and_then(|entry| entry.writer.take())
                {
                    writer.close("logging_stopped");
                }
            }
            _ => {}
        }
        if result.is_err() {
            match previous {
                Some(previous) => manager.manual.insert(tab_id.to_string(), previous),
                None => manager.manual.remove(tab_id),
            };
        }
        self.sync_active(&manager);
        drop(manager);
        emit_status(app, self);
        result?;
        self.status()
    }

    pub(super) fn end_session(&self, app: &AppHandle, tab_id: &str) {
        let writer = self.manager.lock().ok().and_then(|mut manager| {
            let writer = manager
                .sessions
                .remove(tab_id)
                .and_then(|entry| entry.writer);
            self.sync_active(&manager);
            writer
        });
        if let Some(writer) = writer {
            writer.close("session_end");
        }
        emit_status(app, self);
    }

    /// Ends every log and waits a short while for them to be written out;
    /// tTerm is quitting.
    pub(super) fn shutdown(&self) {
        let writers: Vec<LogHandle> = match self.manager.lock() {
            Ok(mut manager) => {
                let writers = manager
                    .sessions
                    .values_mut()
                    .filter_map(|entry| entry.writer.take())
                    .collect();
                self.sync_active(&manager);
                writers
            }
            Err(_) => Vec::new(),
        };
        for writer in writers {
            writer.close("session_end");
        }
        worker::wait_for_closing(SHUTDOWN_WAIT);
    }

    /// Has the log `id`, if it is being written, write out what it has
    /// buffered, so reading it shows everything logged so far.
    pub(super) fn flush_log(&self, id: &str) {
        let flushed = self.manager.lock().ok().and_then(|manager| {
            manager
                .sessions
                .values()
                .filter_map(|entry| entry.writer.as_ref())
                .find(|writer| writer.id == id)
                .and_then(LogHandle::request_flush)
        });
        if let Some(flushed) = flushed {
            worker::wait_flushed(&flushed);
        }
    }

    pub(super) fn status(&self) -> Result<TerminalLogStatus, String> {
        let (enabled, directory, mut recording_tab_ids, last_error) = {
            let manager = self.lock_manager()?;
            let (enabled, directory) = match manager.config.as_ref() {
                Some(config) => (config.record_by_default, config.files.directory.clone()),
                None => (false, crate::config::paths::get_config_path()?.join("logs")),
            };
            let recording: Vec<String> = manager
                .sessions
                .iter()
                .filter(|(_, entry)| entry.writer.is_some())
                .map(|(tab_id, _)| tab_id.clone())
                .collect();
            (enabled, directory, recording, manager.last_error.clone())
        };
        recording_tab_ids.sort();
        Ok(TerminalLogStatus {
            enabled,
            directory: directory.to_string_lossy().into_owned(),
            active_sessions: recording_tab_ids.len(),
            recording_tab_ids,
            total_size_bytes: self.disk_usage.load(Ordering::Relaxed),
            last_error,
        })
    }

    /// The log directory, the retention settings, and the files no one may
    /// touch: those being written or compressed.
    pub(super) fn directory_state(&self) -> Result<(PathBuf, Retention, HashSet<PathBuf>), String> {
        let (directory, retention) = {
            let manager = self.lock_manager()?;
            match manager.config.as_ref() {
                Some(config) => (config.files.directory.clone(), config.retention),
                None => (
                    crate::config::paths::get_config_path()?.join("logs"),
                    Retention::default(),
                ),
            }
        };
        Ok((directory, retention, rotating::in_use()))
    }
}

fn validate_directory(directory: &Path) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|err| {
        format!(
            "Failed to create terminal log directory '{}': {err}",
            directory.display()
        )
    })?;
    let probe = directory.join(format!(".tterm-write-test-{}", uuid::Uuid::new_v4()));
    File::create(&probe)
        .and_then(|mut file| file.write_all(b"ok"))
        .map_err(|err| format!("Terminal log directory is not writable: {err}"))?;
    fs::remove_file(&probe)
        .map_err(|err| format!("Failed to remove terminal log write test: {err}"))?;
    Ok(())
}
