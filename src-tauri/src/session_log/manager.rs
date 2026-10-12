//! The sessions being logged, the settings they are logged with, and the
//! cleanup of old logs.

use super::retention::{self, Retention};
use super::rotating::COMPRESSING;
use super::writer::{lock_session_writer, SessionWriter};
use super::{
    emit_error, emit_start_error, emit_status, LogConfig, SessionLogPolicy, SessionLogState,
    SessionMetadata, TerminalLogStatus, CLEANUP_INTERVAL,
};
use crate::config::AppConfig;
use crate::core::session::SessionPlan;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Instant, SystemTime};
use tauri::{AppHandle, Emitter, Manager};

struct SessionEntry {
    metadata: SessionMetadata,
    policy: SessionLogPolicy,
    rows: u16,
    cols: u16,
    writer: Option<Arc<Mutex<SessionWriter>>>,
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

    fn remember_log(&mut self, tab_id: &str, writer: &Arc<Mutex<SessionWriter>>) {
        let id = lock_session_writer(writer).id.clone();
        self.logged.entry(tab_id.to_string()).or_default().push(id);
    }

    fn has_writers(&self) -> bool {
        self.sessions.values().any(|entry| entry.writer.is_some())
    }

    fn create_writer(
        &self,
        app: &AppHandle,
        entry: &SessionEntry,
        event: &str,
    ) -> Result<Arc<Mutex<SessionWriter>>, String> {
        let config = self
            .config
            .as_ref()
            .ok_or("Terminal logging is not configured")?;
        let writer = Arc::new(Mutex::new(SessionWriter::create(
            app,
            config,
            entry.metadata.clone(),
            entry.rows,
            entry.cols,
        )?));
        // The size lets a replay start at the size the session had.
        lock_session_writer(&writer).record_event(
            event,
            serde_json::json!({ "rows": entry.rows, "cols": entry.cols }),
        )?;
        Ok(writer)
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
                    lock_session_writer(&writer).record_input = config.record_input;
                    entry.writer = Some(writer);
                } else {
                    let event = if records {
                        "logging_reconfigured"
                    } else {
                        "logging_disabled"
                    };
                    if let Err(err) = lock_session_writer(&writer).close_with(event) {
                        error = Some(err);
                    }
                }
            }
            if records && manager.sessions[&tab_id].writer.is_none() {
                match manager.create_writer(app, &manager.sessions[&tab_id], "logging_enabled") {
                    Ok(writer) => {
                        manager.remember_log(&tab_id, &writer);
                        manager
                            .sessions
                            .get_mut(&tab_id)
                            .expect("listed above")
                            .writer = Some(writer)
                    }
                    Err(err) => error = Some(err),
                }
            }
        }
        manager.last_error = error.clone();
        self.sync_active(&manager);
        drop(manager);
        emit_status(app, self);
        if files_changed || retention_changed {
            self.schedule_cleanup(app, true);
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
        if let Some(previous) = manager.sessions.remove(tab_id) {
            if let Some(writer) = previous.writer {
                let _ = lock_session_writer(&writer).finish();
            }
        }
        let mut failure = None;
        let writer = if manager.records(tab_id, entry.policy) {
            match manager.create_writer(app, &entry, "session_start") {
                Ok(writer) => {
                    manager.remember_log(tab_id, &writer);
                    Some(writer)
                }
                Err(err) => {
                    manager.last_error = Some(err.clone());
                    failure = Some(err);
                    None
                }
            }
        } else {
            None
        };
        manager
            .sessions
            .insert(tab_id.to_string(), SessionEntry { writer, ..entry });
        self.sync_active(&manager);
        drop(manager);
        if let Some(err) = failure {
            emit_start_error(app, tab_id, &err);
        }
        emit_status(app, self);
    }

    fn writer(&self, app: &AppHandle, tab_id: &str) -> Option<Arc<Mutex<SessionWriter>>> {
        match self.manager.lock() {
            Ok(manager) => manager
                .sessions
                .get(tab_id)
                .and_then(|entry| entry.writer.clone()),
            Err(_) => {
                emit_error(app, tab_id, "Terminal log state is unavailable");
                None
            }
        }
    }

    fn report(&self, app: &AppHandle, tab_id: &str, result: Result<(), String>) {
        if let Err(err) = result {
            self.set_error(err.clone());
            emit_error(app, tab_id, &err);
        }
    }

    pub(super) fn record_bytes(&self, app: &AppHandle, tab_id: &str, direction: &str, data: &[u8]) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }
        let Some(writer) = self.writer(app, tab_id) else {
            return;
        };
        let result = lock_session_writer(&writer).record_bytes(direction, data);
        self.report(app, tab_id, result);
    }

    pub(super) fn record_event(
        &self,
        app: &AppHandle,
        tab_id: &str,
        event: &str,
        detail: serde_json::Value,
    ) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }
        let Some(writer) = self.writer(app, tab_id) else {
            return;
        };
        let result = lock_session_writer(&writer).record_event(event, detail);
        self.report(app, tab_id, result);
    }

    pub(super) fn resize(&self, app: &AppHandle, tab_id: &str, rows: u16, cols: u16) {
        let writer = match self.manager.lock() {
            Ok(mut manager) => manager.sessions.get_mut(tab_id).and_then(|entry| {
                entry.rows = rows;
                entry.cols = cols;
                entry.writer.clone()
            }),
            Err(_) => return,
        };
        if let Some(writer) = writer {
            let result = lock_session_writer(&writer).resize(rows, cols);
            self.report(app, tab_id, result);
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
        match (
            recording,
            manager
                .sessions
                .get(tab_id)
                .map(|entry| entry.writer.clone()),
        ) {
            (true, Some(None)) => {
                match manager.create_writer(app, &manager.sessions[tab_id], "logging_started") {
                    Ok(writer) => {
                        manager.remember_log(tab_id, &writer);
                        manager
                            .sessions
                            .get_mut(tab_id)
                            .expect("found above")
                            .writer = Some(writer);
                    }
                    Err(err) => result = Err(err),
                }
            }
            (false, Some(Some(writer))) => {
                manager
                    .sessions
                    .get_mut(tab_id)
                    .expect("found above")
                    .writer = None;
                result = lock_session_writer(&writer).close_with("logging_stopped");
            }
            _ => {}
        }
        if result.is_err() && recording {
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
            if let Err(err) = lock_session_writer(&writer).finish() {
                emit_error(app, tab_id, &err);
            }
        }
        emit_status(app, self);
        self.schedule_cleanup(app, false);
    }

    pub(super) fn status(&self) -> Result<TerminalLogStatus, String> {
        let (enabled, directory, mut recording_tab_ids, last_error) = {
            let manager = self.lock_manager()?;
            let (enabled, directory) = match manager.config.as_ref() {
                Some(config) => (config.record_by_default, config.files.directory.clone()),
                None => (false, crate::config::get_config_path()?.join("logs")),
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
            total_size_bytes: directory_size(&directory),
            last_error,
        })
    }

    /// Removes logs the retention settings no longer keep, on a background
    /// thread. Unless `force`d, runs at most once per `CLEANUP_INTERVAL`.
    /// Never takes the manager lock on the caller's thread, so it is safe to
    /// call while a log is closing.
    pub(super) fn schedule_cleanup(&self, app: &AppHandle, force: bool) {
        {
            let mut last = match self.last_cleanup.lock() {
                Ok(last) => last,
                Err(poisoned) => poisoned.into_inner(),
            };
            if !force && last.is_some_and(|last| last.elapsed() < CLEANUP_INTERVAL) {
                return;
            }
            *last = Some(Instant::now());
        }
        if self.cleanup_running.swap(true, Ordering::AcqRel) {
            self.cleanup_again.store(true, Ordering::Release);
            return;
        }
        let app = app.clone();
        std::thread::spawn(move || {
            let state = app.state::<SessionLogState>();
            loop {
                state.clean_now(&app);
                if state.cleanup_again.swap(false, Ordering::AcqRel) {
                    continue;
                }
                state.cleanup_running.store(false, Ordering::Release);
                // A request that came in just before the flag dropped.
                if state.cleanup_again.swap(false, Ordering::AcqRel)
                    && !state.cleanup_running.swap(true, Ordering::AcqRel)
                {
                    continue;
                }
                break;
            }
        });
    }

    /// The log directory, the retention settings, and the files no one may
    /// touch: those being written or compressed.
    pub(super) fn directory_state(&self) -> Result<(PathBuf, Retention, HashSet<PathBuf>), String> {
        let (directory, retention, writers) = {
            let manager = self.lock_manager()?;
            let (directory, retention) = match manager.config.as_ref() {
                Some(config) => (config.files.directory.clone(), config.retention),
                None => (
                    crate::config::get_config_path()?.join("logs"),
                    Retention::default(),
                ),
            };
            let writers: Vec<_> = manager
                .sessions
                .values()
                .filter_map(|entry| entry.writer.clone())
                .collect();
            (directory, retention, writers)
        };
        let mut in_use: HashSet<PathBuf> = writers
            .iter()
            .flat_map(|writer| lock_session_writer(writer).open_paths().collect::<Vec<_>>())
            .collect();
        if let Ok(compressing) = COMPRESSING.lock() {
            in_use.extend(compressing.iter().cloned());
        }
        Ok((directory, retention, in_use))
    }

    fn clean_now(&self, app: &AppHandle) {
        let Ok((directory, retention, in_use)) = self.directory_state() else {
            return;
        };
        if retention.is_off() {
            return;
        }
        match retention::clean(&directory, retention, &in_use, SystemTime::now()) {
            Ok(report) if report.removed > 0 => emit_status(app, self),
            Ok(_) => {}
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
