mod library;
mod manager;
mod naming;
mod plain;
mod retention;
mod rotating;
mod screen;
#[cfg(test)]
mod tests;
mod writer;

use crate::config::AppConfig;
use crate::core::blocking::run_blocking;
use crate::core::session::SessionPlan;
use crate::core::state::SessionKind;
use manager::LogManager;
use naming::template_parts;
use retention::Retention;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

const LOG_FORMAT_RAW: &str = "raw";
const LOG_FORMAT_PLAIN: &str = "plain";
const LOG_FORMAT_BOTH: &str = "both";
const MIN_FILE_SIZE_MB: u32 = 1;
const MAX_FILE_SIZE_MB: u32 = 1024;
const MAX_RETENTION_DAYS: u32 = 3650;
const MAX_TOTAL_SIZE_MB: u32 = 1024 * 1024;
/// Input lines and events wait for the output lines that came before them;
/// past this many, the oldest are written anyway.
const MAX_QUEUED_PLAIN_LINES: usize = 10_000;
/// Cleanup after a log closes runs at most this often.
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

/// Whether a connection's sessions are logged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionLogPolicy {
    /// As the logging setting says.
    #[default]
    Default,
    Always,
    Never,
}

impl SessionLogPolicy {
    pub fn from_label(label: Option<&str>) -> Self {
        match label.map(str::trim) {
            Some("always") => Self::Always,
            Some("never") => Self::Never,
            _ => Self::Default,
        }
    }
}

/// Where and how log files are written; changing any of it starts new files.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LogFiles {
    directory: PathBuf,
    format: String,
    name_template: String,
    max_file_size_bytes: u64,
    compress: bool,
    plain_timestamps: bool,
}

impl LogFiles {
    fn wants_raw(&self) -> bool {
        self.format == LOG_FORMAT_RAW || self.format == LOG_FORMAT_BOTH
    }

    fn wants_plain(&self) -> bool {
        self.format == LOG_FORMAT_PLAIN || self.format == LOG_FORMAT_BOTH
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LogConfig {
    /// Log sessions whose connection does not say otherwise.
    record_by_default: bool,
    record_input: bool,
    files: LogFiles,
    retention: Retention,
}

impl LogConfig {
    fn from_app_config(config: &AppConfig) -> Result<Self, String> {
        let format = match config.terminal_log_format.as_str() {
            LOG_FORMAT_RAW | LOG_FORMAT_PLAIN | LOG_FORMAT_BOTH => {
                config.terminal_log_format.clone()
            }
            _ => return Err("Unsupported terminal log format".to_string()),
        };
        let name_template = config.terminal_log_name_template.trim();
        if name_template.is_empty() {
            return Err("Terminal log file name cannot be empty".to_string());
        }
        if name_template.chars().count() > 180 {
            return Err("Terminal log file name is too long".to_string());
        }
        if template_parts(name_template).count() > library::MAX_FOLDER_DEPTH + 1 {
            return Err(format!(
                "Terminal log file name can have at most {} folders",
                library::MAX_FOLDER_DEPTH
            ));
        }

        let size_mb = config
            .terminal_log_max_file_size_mb
            .clamp(MIN_FILE_SIZE_MB, MAX_FILE_SIZE_MB);
        let directory = if config.terminal_log_directory.trim().is_empty() {
            crate::config::get_config_path()?.join("logs")
        } else {
            PathBuf::from(config.terminal_log_directory.trim())
        };

        Ok(Self {
            record_by_default: config.terminal_log_enabled,
            record_input: config.terminal_log_record_input,
            files: LogFiles {
                directory,
                format,
                name_template: name_template.to_string(),
                max_file_size_bytes: u64::from(size_mb) * 1024 * 1024,
                compress: config.terminal_log_compress,
                plain_timestamps: config.terminal_log_plain_timestamps,
            },
            retention: Retention {
                max_age_days: config.terminal_log_retention_days.min(MAX_RETENTION_DAYS),
                max_total_bytes: u64::from(config.terminal_log_max_total_mb.min(MAX_TOTAL_SIZE_MB))
                    * 1024
                    * 1024,
            },
        })
    }

    /// Whether a session is logged: what the user chose for its tab, else
    /// what its connection says, else the default.
    fn records(&self, policy: SessionLogPolicy, manual: Option<bool>) -> bool {
        manual.unwrap_or(match policy {
            SessionLogPolicy::Always => true,
            SessionLogPolicy::Never => false,
            SessionLogPolicy::Default => self.record_by_default,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionMetadata {
    tab_id: String,
    session_nonce: u32,
    session_type: String,
    profile: String,
    host: String,
    port: u16,
    username: String,
    started_at_ms: u64,
}

impl SessionMetadata {
    fn new(tab_id: &str, session_nonce: u32, plan: &SessionPlan) -> Self {
        Self {
            tab_id: tab_id.to_string(),
            session_nonce,
            session_type: match plan.kind {
                SessionKind::Terminal => "local",
                SessionKind::Ssh => "ssh",
            }
            .to_string(),
            profile: plan.profile_name.clone(),
            host: plan.host.clone().unwrap_or_else(|| "local".to_string()),
            port: plan.port,
            username: plan.username.clone().unwrap_or_default(),
            started_at_ms: now_unix_ms(),
        }
    }
}

#[derive(Default)]
pub struct SessionLogState {
    manager: Mutex<LogManager>,
    /// Some session is being logged; lets output skip the manager lock when
    /// none is.
    active: AtomicBool,
    last_cleanup: Mutex<Option<Instant>>,
    cleanup_running: AtomicBool,
    cleanup_again: AtomicBool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLogStatus {
    /// Sessions are logged unless their connection or tab says otherwise.
    enabled: bool,
    directory: String,
    active_sessions: usize,
    recording_tab_ids: Vec<String>,
    total_size_bytes: u64,
    last_error: Option<String>,
}

pub fn start_session(
    app: &AppHandle,
    tab_id: &str,
    session_nonce: u32,
    plan: &SessionPlan,
    rows: u16,
    cols: u16,
) {
    app.state::<SessionLogState>()
        .start_session(app, tab_id, session_nonce, plan, rows, cols)
}

pub fn record_input(app: &AppHandle, tab_id: &str, data: &[u8]) {
    app.state::<SessionLogState>()
        .record_bytes(app, tab_id, "input", data);
}

pub fn record_output(app: &AppHandle, tab_id: &str, data: &[u8]) {
    app.state::<SessionLogState>()
        .record_bytes(app, tab_id, "output", data);
}

pub fn record_resize(app: &AppHandle, tab_id: &str, rows: u16, cols: u16) {
    app.state::<SessionLogState>()
        .resize(app, tab_id, rows, cols);
}

pub fn record_credential_injection(app: &AppHandle, tab_id: &str) {
    app.state::<SessionLogState>().record_event(
        app,
        tab_id,
        "credential_injected",
        serde_json::json!({ "redacted": true }),
    );
}

pub fn end_session(app: &AppHandle, tab_id: &str) {
    app.state::<SessionLogState>().end_session(app, tab_id);
}

#[tauri::command]
pub fn get_terminal_log_status(
    state: State<'_, SessionLogState>,
) -> Result<TerminalLogStatus, String> {
    state.status()
}

/// The logs a tab has written since tTerm started, oldest first.
#[tauri::command]
pub fn terminal_log_ids_for_tab(
    state: State<'_, SessionLogState>,
    tab_id: String,
) -> Result<Vec<String>, String> {
    Ok(state
        .lock_manager()?
        .logged
        .get(&tab_id)
        .cloned()
        .unwrap_or_default())
}

/// Starts or stops logging one tab, whatever the settings say, until the
/// tab closes.
#[tauri::command]
pub fn set_terminal_log_recording(
    app: AppHandle,
    state: State<'_, SessionLogState>,
    tab_id: String,
    recording: bool,
) -> Result<TerminalLogStatus, String> {
    state.set_recording(&app, &tab_id, recording)
}

/// The saved sessions in the log directory, newest first.
#[tauri::command]
pub async fn list_terminal_logs(app: AppHandle) -> Result<Vec<library::LogSession>, String> {
    let (directory, _, in_use) = app.state::<SessionLogState>().directory_state()?;
    run_blocking(move || library::list(&directory, &in_use)).await
}

/// A session's raw log as replay frames (see `library::load_recording`).
#[tauri::command]
pub async fn load_terminal_log_recording(app: AppHandle, id: String) -> Result<Response, String> {
    let (directory, _, _) = app.state::<SessionLogState>().directory_state()?;
    run_blocking(move || library::load_recording(&directory, &id))
        .await
        .map(Response::new)
}

/// A session's plain-text log as UTF-8 bytes.
#[tauri::command]
pub async fn load_terminal_log_text(app: AppHandle, id: String) -> Result<Response, String> {
    let (directory, _, _) = app.state::<SessionLogState>().directory_state()?;
    run_blocking(move || library::load_text(&directory, &id))
        .await
        .map(Response::new)
}

#[tauri::command]
pub async fn export_terminal_log_asciicast(
    app: AppHandle,
    id: String,
    path: String,
) -> Result<(), String> {
    let (directory, _, _) = app.state::<SessionLogState>().directory_state()?;
    run_blocking(move || library::export_asciicast(&directory, &id, Path::new(&path))).await
}

#[tauri::command]
pub async fn delete_terminal_log(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<SessionLogState>();
    let (directory, _, in_use) = state.directory_state()?;
    run_blocking(move || library::delete(&directory, &id, &in_use)).await?;
    emit_status(&app, &state);
    Ok(())
}

/// Deletes several sessions; one that cannot be deleted (such as one still
/// being logged) is reported and does not stop the rest.
#[tauri::command]
pub async fn delete_terminal_logs(
    app: AppHandle,
    ids: Vec<String>,
) -> Result<library::DeleteReport, String> {
    let state = app.state::<SessionLogState>();
    let (directory, _, in_use) = state.directory_state()?;
    let report = run_blocking(move || Ok(library::delete_many(&directory, &ids, &in_use))).await?;
    emit_status(&app, &state);
    Ok(report)
}

#[tauri::command]
pub async fn reveal_terminal_log(app: AppHandle, id: String) -> Result<(), String> {
    let (directory, _, _) = app.state::<SessionLogState>().directory_state()?;
    let path = run_blocking(move || library::first_file(&directory, &id)).await?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|error| format!("Failed to show terminal log: {error}"))
}

/// Saves what a terminal shows (its text) to a file the user picked.
#[tauri::command]
pub async fn save_terminal_contents(path: String, contents: String) -> Result<(), String> {
    run_blocking(move || {
        fs::write(&path, contents).map_err(|error| format!("Failed to save '{path}': {error}"))
    })
    .await
}

#[tauri::command]
pub fn open_terminal_log_directory(
    app: AppHandle,
    state: State<'_, SessionLogState>,
) -> Result<(), String> {
    let directory = {
        let manager = state.lock_manager()?;
        manager
            .config
            .as_ref()
            .map(|config| config.files.directory.clone())
            .unwrap_or(crate::config::get_config_path()?.join("logs"))
    };

    fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed to create terminal log directory: {error}"))?;
    app.opener()
        .open_path(directory.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|error| format!("Failed to open terminal log directory: {error}"))
}

#[tauri::command]
pub fn retry_terminal_logging(
    app: AppHandle,
    state: State<'_, SessionLogState>,
) -> Result<TerminalLogStatus, String> {
    let config = crate::config::load_config_file()?;
    {
        let mut manager = state.lock_manager()?;
        manager.config = None;
    }
    state.apply_config(&app, &config)?;
    state.status()
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn emit_error(app: &AppHandle, tab_id: &str, message: &str) {
    let _ = app.emit_to(
        tauri::EventTarget::any(),
        "terminal-log-error",
        serde_json::json!({ "tabId": tab_id, "message": message }),
    );
}

/// A session's log could not start; the session runs without one.
fn emit_start_error(app: &AppHandle, tab_id: &str, message: &str) {
    let _ = app.emit_to(
        tauri::EventTarget::any(),
        "terminal-log-error",
        serde_json::json!({ "tabId": tab_id, "message": message, "phase": "start" }),
    );
}

fn emit_status(app: &AppHandle, state: &SessionLogState) {
    if let Ok(status) = state.status() {
        let _ = app.emit_to(tauri::EventTarget::any(), "terminal-log-status", status);
    }
}
