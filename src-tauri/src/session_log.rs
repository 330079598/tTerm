use crate::config::AppConfig;
use crate::core::session::SessionPlan;
use crate::core::state::SessionKind;
use base64::Engine;
use chrono::Local;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use vte::{Params, Perform};

const LOG_FORMAT_RAW: &str = "raw";
const LOG_FORMAT_PLAIN: &str = "plain";
const LOG_FORMAT_BOTH: &str = "both";
const MIN_FILE_SIZE_MB: u32 = 1;
const MAX_FILE_SIZE_MB: u32 = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
struct LogConfig {
    enabled: bool,
    directory: PathBuf,
    format: String,
    name_template: String,
    max_file_size_bytes: u64,
    compress: bool,
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

        let size_mb = config
            .terminal_log_max_file_size_mb
            .clamp(MIN_FILE_SIZE_MB, MAX_FILE_SIZE_MB);
        let directory = if config.terminal_log_directory.trim().is_empty() {
            crate::config::get_config_path()?.join("logs")
        } else {
            PathBuf::from(config.terminal_log_directory.trim())
        };

        Ok(Self {
            enabled: config.terminal_log_enabled,
            directory,
            format,
            name_template: name_template.to_string(),
            max_file_size_bytes: u64::from(size_mb) * 1024 * 1024,
            compress: config.terminal_log_compress,
        })
    }

    fn wants_raw(&self) -> bool {
        self.format == LOG_FORMAT_RAW || self.format == LOG_FORMAT_BOTH
    }

    fn wants_plain(&self) -> bool {
        self.format == LOG_FORMAT_PLAIN || self.format == LOG_FORMAT_BOTH
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

struct RotatingFile {
    file: Option<File>,
    path: PathBuf,
    directory: PathBuf,
    base_name: String,
    extension: &'static str,
    part: u32,
    bytes_written: u64,
    max_bytes: u64,
    compress: bool,
    app: AppHandle,
    header: Vec<u8>,
}

impl RotatingFile {
    fn create(
        app: &AppHandle,
        directory: &Path,
        base_name: &str,
        extension: &'static str,
        max_bytes: u64,
        compress: bool,
        header: &[u8],
    ) -> Result<Self, String> {
        let path = part_path(directory, base_name, extension, 0);
        let mut file = open_new_file(&path)?;
        file.write_all(header)
            .map_err(|err| format!("Failed to write terminal log header: {err}"))?;
        Ok(Self {
            file: Some(file),
            path,
            directory: directory.to_path_buf(),
            base_name: base_name.to_string(),
            extension,
            part: 0,
            bytes_written: header.len() as u64,
            max_bytes,
            compress,
            app: app.clone(),
            header: header.to_vec(),
        })
    }

    fn write_event(&mut self, data: &[u8]) -> Result<(), String> {
        if self.bytes_written > 0 && self.bytes_written + data.len() as u64 > self.max_bytes {
            self.rotate()?;
        }
        let file = self.file.as_mut().ok_or("Terminal log file is closed")?;
        file.write_all(data).map_err(|err| {
            format!(
                "Failed to write terminal log '{}': {err}",
                self.path.display()
            )
        })?;
        self.bytes_written += data.len() as u64;
        Ok(())
    }

    fn rotate(&mut self) -> Result<(), String> {
        self.finish_current()?;
        self.part = self.part.saturating_add(1);
        self.path = part_path(&self.directory, &self.base_name, self.extension, self.part);
        let mut file = open_new_file(&self.path)?;
        file.write_all(&self.header)
            .map_err(|err| format!("Failed to write terminal log header: {err}"))?;
        self.file = Some(file);
        self.bytes_written = self.header.len() as u64;
        Ok(())
    }

    fn finish_current(&mut self) -> Result<(), String> {
        let Some(mut file) = self.file.take() else {
            return Ok(());
        };
        file.flush().and_then(|_| file.sync_all()).map_err(|err| {
            format!(
                "Failed to flush terminal log '{}': {err}",
                self.path.display()
            )
        })?;
        drop(file);
        if self.compress && self.bytes_written > 0 {
            spawn_compression(self.app.clone(), self.path.clone());
        }
        Ok(())
    }
}

impl Drop for RotatingFile {
    fn drop(&mut self) {
        let _ = self.finish_current();
    }
}

struct SessionWriter {
    started: Instant,
    sequence: u64,
    closed: bool,
    raw: Option<RotatingFile>,
    plain: Option<RotatingFile>,
    plain_input: PlainStream,
    plain_output: PlainStream,
}

struct PlainStream {
    parser: vte::Parser,
    screen: PlainScreen,
}

impl PlainStream {
    fn new(commit_on_cr: bool) -> Self {
        Self {
            parser: vte::Parser::new(),
            screen: PlainScreen::new(commit_on_cr),
        }
    }

    fn advance(&mut self, data: &[u8]) -> Vec<String> {
        self.parser.advance(&mut self.screen, data);
        std::mem::take(&mut self.screen.completed)
    }

    fn finish(&mut self) -> Vec<String> {
        if self
            .screen
            .line
            .iter()
            .any(|character| !character.is_whitespace())
        {
            self.screen.complete_line();
        }
        std::mem::take(&mut self.screen.completed)
    }
}

struct PlainScreen {
    line: Vec<char>,
    cursor: usize,
    completed: Vec<String>,
    commit_on_cr: bool,
    just_committed_cr: bool,
}

impl PlainScreen {
    fn new(commit_on_cr: bool) -> Self {
        Self {
            line: Vec::new(),
            cursor: 0,
            completed: Vec::new(),
            commit_on_cr,
            just_committed_cr: false,
        }
    }

    fn complete_line(&mut self) {
        let line = self.line.iter().collect::<String>();
        self.completed.push(line.trim_end().to_string());
        self.line.clear();
        self.cursor = 0;
    }

    fn move_cursor_forward(&mut self, count: usize) {
        self.cursor = self.cursor.saturating_add(count);
        if self.cursor > self.line.len() {
            self.line.resize(self.cursor, ' ');
        }
    }

    fn first_param(params: &Params, default: u16) -> usize {
        params
            .iter()
            .next()
            .and_then(|param| param.first())
            .copied()
            .filter(|value| *value != 0)
            .unwrap_or(default) as usize
    }
}

impl Perform for PlainScreen {
    fn print(&mut self, character: char) {
        self.just_committed_cr = false;
        if self.cursor < self.line.len() {
            self.line[self.cursor] = character;
        } else {
            self.line.resize(self.cursor, ' ');
            self.line.push(character);
        }
        self.cursor = self.cursor.saturating_add(1);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' if self.commit_on_cr && self.just_committed_cr => {
                self.just_committed_cr = false;
            }
            b'\n' => self.complete_line(),
            b'\r' if self.commit_on_cr => {
                if self.line.is_empty() {
                    self.completed.push("<Enter>".to_string());
                } else {
                    self.complete_line();
                }
                self.just_committed_cr = true;
            }
            b'\r' => self.cursor = 0,
            0x08 | 0x7f if self.commit_on_cr => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    if self.cursor < self.line.len() {
                        self.line.remove(self.cursor);
                    }
                }
            }
            0x08 | 0x7f => self.cursor = self.cursor.saturating_sub(1),
            b'\t' => {
                let spaces = 8 - (self.cursor % 8);
                self.move_cursor_forward(spaces);
            }
            0x03 if self.commit_on_cr => {
                if !self.line.is_empty() {
                    self.complete_line();
                }
                self.completed.push("<Ctrl+C>".to_string());
            }
            0x04 if self.commit_on_cr => self.completed.push("<Ctrl+D>".to_string()),
            0x0c if self.commit_on_cr => self.completed.push("<Ctrl+L>".to_string()),
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        match action {
            'C' | 'a' => self.move_cursor_forward(Self::first_param(params, 1)),
            'D' => {
                self.cursor = self.cursor.saturating_sub(Self::first_param(params, 1));
            }
            'G' | '`' => {
                self.cursor = Self::first_param(params, 1).saturating_sub(1);
            }
            'H' | 'f' => {
                let column = params
                    .iter()
                    .nth(1)
                    .and_then(|param| param.first())
                    .copied()
                    .filter(|value| *value != 0)
                    .unwrap_or(1);
                self.cursor = usize::from(column.saturating_sub(1));
            }
            'J' => {
                let mode = params
                    .iter()
                    .next()
                    .and_then(|param| param.first())
                    .copied()
                    .unwrap_or(0);
                if mode == 2 || mode == 3 {
                    self.line.clear();
                    self.cursor = 0;
                } else if mode == 0 {
                    self.line.truncate(self.cursor);
                }
            }
            'K' => {
                let mode = params
                    .iter()
                    .next()
                    .and_then(|param| param.first())
                    .copied()
                    .unwrap_or(0);
                match mode {
                    0 => self.line.truncate(self.cursor),
                    1 => {
                        let end = self.cursor.min(self.line.len().saturating_sub(1));
                        self.line
                            .iter_mut()
                            .take(end + 1)
                            .for_each(|value| *value = ' ');
                    }
                    2 => {
                        self.line.clear();
                        self.cursor = 0;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl SessionWriter {
    fn create(
        app: &AppHandle,
        config: &LogConfig,
        metadata: SessionMetadata,
    ) -> Result<Self, String> {
        fs::create_dir_all(&config.directory).map_err(|err| {
            format!(
                "Failed to create terminal log directory '{}': {err}",
                config.directory.display()
            )
        })?;
        let rendered_name = render_name(&config.name_template, &metadata);
        let base_name = unique_base_name(&config.directory, &rendered_name, config);
        let raw_header = with_newline(
            serde_json::to_vec(&serde_json::json!({
                "type": "header",
                "version": 1,
                "metadata": &metadata,
            }))
            .map_err(|err| format!("Failed to serialize terminal log header: {err}"))?,
        );
        let plain_header = format!(
            "# tTerm session log\n# type={} profile={} host={} port={} username={} startedAt={}\n",
            metadata.session_type,
            metadata.profile,
            metadata.host,
            metadata.port,
            metadata.username,
            metadata.started_at_ms
        );
        let raw = if config.wants_raw() {
            Some(RotatingFile::create(
                app,
                &config.directory,
                &base_name,
                "tlog",
                config.max_file_size_bytes,
                config.compress,
                &raw_header,
            )?)
        } else {
            None
        };
        let plain = if config.wants_plain() {
            Some(RotatingFile::create(
                app,
                &config.directory,
                &base_name,
                "log",
                config.max_file_size_bytes,
                config.compress,
                plain_header.as_bytes(),
            )?)
        } else {
            None
        };

        let writer = Self {
            started: Instant::now(),
            sequence: 0,
            closed: false,
            raw,
            plain,
            plain_input: PlainStream::new(true),
            plain_output: PlainStream::new(false),
        };
        Ok(writer)
    }

    fn record_bytes(&mut self, direction: &str, data: &[u8]) -> Result<(), String> {
        if self.closed || data.is_empty() {
            return Ok(());
        }
        self.sequence = self.sequence.saturating_add(1);
        let elapsed_us = self.started.elapsed().as_micros().min(u64::MAX as u128) as u64;
        let timestamp_ms = now_unix_ms();
        if let Some(raw) = self.raw.as_mut() {
            let event = serde_json::to_vec(&serde_json::json!({
                "type": "data",
                "sequence": self.sequence,
                "timestampMs": timestamp_ms,
                "elapsedUs": elapsed_us,
                "direction": direction,
                "encoding": "base64",
                "data": base64::engine::general_purpose::STANDARD.encode(data),
                "crc32": crc32fast::hash(data),
            }))
            .map_err(|err| format!("Failed to serialize terminal log event: {err}"))?;
            raw.write_event(&with_newline(event))?;
        }
        if let Some(plain) = self.plain.as_mut() {
            let lines = if direction == "input" {
                self.plain_input.advance(data)
            } else {
                self.plain_output.advance(data)
            };
            write_plain_lines(plain, direction, lines)?;
        }
        Ok(())
    }

    fn record_event(&mut self, event_type: &str, detail: serde_json::Value) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        self.sequence = self.sequence.saturating_add(1);
        let timestamp_ms = now_unix_ms();
        if let Some(raw) = self.raw.as_mut() {
            let event = serde_json::to_vec(&serde_json::json!({
                "type": event_type,
                "sequence": self.sequence,
                "timestampMs": timestamp_ms,
                "elapsedUs": self.started.elapsed().as_micros().min(u64::MAX as u128) as u64,
                "detail": detail,
            }))
            .map_err(|err| format!("Failed to serialize terminal log event: {err}"))?;
            raw.write_event(&with_newline(event))?;
        }
        if let Some(plain) = self.plain.as_mut() {
            let event = format!(
                "[{}] [EVENT] {} {}\n",
                Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                event_type,
                detail
            );
            plain.write_event(event.as_bytes())?;
        }
        Ok(())
    }

    fn close_with(&mut self, event_type: &str) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        if let Some(plain) = self.plain.as_mut() {
            write_plain_lines(plain, "input", self.plain_input.finish())?;
            write_plain_lines(plain, "output", self.plain_output.finish())?;
        }
        self.record_event(event_type, serde_json::json!({}))?;
        self.closed = true;
        if let Some(mut raw) = self.raw.take() {
            raw.finish_current()?;
        }
        if let Some(mut plain) = self.plain.take() {
            plain.finish_current()?;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), String> {
        self.close_with("session_end")
    }
}

fn write_plain_lines(
    plain: &mut RotatingFile,
    direction: &str,
    lines: Vec<String>,
) -> Result<(), String> {
    for line in lines {
        if line.is_empty() {
            plain.write_event(b"\n")?;
            continue;
        }
        let event = format!(
            "[{}] [{}] {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            direction.to_ascii_uppercase(),
            line
        );
        plain.write_event(event.as_bytes())?;
    }
    Ok(())
}

struct SessionEntry {
    metadata: SessionMetadata,
    writer: Option<Arc<Mutex<SessionWriter>>>,
}

struct LogManager {
    config: Option<LogConfig>,
    sessions: HashMap<String, SessionEntry>,
    last_error: Option<String>,
}

impl Default for LogManager {
    fn default() -> Self {
        Self {
            config: None,
            sessions: HashMap::new(),
            last_error: None,
        }
    }
}

#[derive(Default)]
pub struct SessionLogState {
    manager: Mutex<LogManager>,
    enabled: AtomicBool,
}

fn lock_session_writer(writer: &Arc<Mutex<SessionWriter>>) -> MutexGuard<'_, SessionWriter> {
    match writer.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLogStatus {
    enabled: bool,
    directory: String,
    active_sessions: usize,
    total_size_bytes: u64,
    last_error: Option<String>,
}

impl SessionLogState {
    fn set_error(&self, message: String) {
        if let Ok(mut manager) = self.manager.lock() {
            manager.last_error = Some(message);
        }
    }

    pub fn validate_config(&self, config: &AppConfig) -> Result<(), String> {
        let config = LogConfig::from_app_config(config)?;
        if config.enabled {
            validate_directory(&config.directory)?;
        }
        Ok(())
    }

    pub fn apply_config(&self, app: &AppHandle, app_config: &AppConfig) -> Result<(), String> {
        let config = LogConfig::from_app_config(app_config)?;
        if config.enabled {
            validate_directory(&config.directory)?;
        }

        let mut manager = self
            .manager
            .lock()
            .map_err(|_| "Terminal log state is unavailable")?;
        if manager.config.as_ref() == Some(&config) {
            return Ok(());
        }
        if config.enabled {
            self.enabled.store(true, Ordering::Relaxed);
        }
        let close_event = if config.enabled {
            "logging_reconfigured"
        } else {
            "logging_disabled"
        };
        let mut close_error = None;
        for entry in manager.sessions.values_mut() {
            if let Some(writer) = entry.writer.take() {
                if let Err(err) = lock_session_writer(&writer).close_with(close_event) {
                    close_error = Some(err);
                }
            }
        }
        if !config.enabled {
            self.enabled.store(false, Ordering::Relaxed);
        }
        manager.config = Some(config.clone());
        manager.last_error = close_error;
        if config.enabled {
            for entry in manager.sessions.values_mut() {
                let writer = Arc::new(Mutex::new(SessionWriter::create(
                    app,
                    &config,
                    entry.metadata.clone(),
                )?));
                lock_session_writer(&writer).record_event("logging_enabled", serde_json::json!({}))?;
                entry.writer = Some(writer);
            }
        }
        drop(manager);
        emit_status(app, self);
        Ok(())
    }

    fn start_session(
        &self,
        app: &AppHandle,
        tab_id: &str,
        session_nonce: u32,
        plan: &SessionPlan,
    ) -> Result<(), String> {
        let metadata = SessionMetadata::new(tab_id, session_nonce, plan);
        let mut manager = self
            .manager
            .lock()
            .map_err(|_| "Terminal log state is unavailable")?;
        if let Some(previous) = manager.sessions.remove(tab_id) {
            if let Some(writer) = previous.writer {
                let _ = lock_session_writer(&writer).finish();
            }
        }
        let writer = match manager.config.as_ref() {
            Some(config) if config.enabled => {
                let writer = Arc::new(Mutex::new(SessionWriter::create(
                    app, config, metadata.clone(),
                )?));
                lock_session_writer(&writer).record_event("session_start", serde_json::json!({}))?;
                Some(writer)
            }
            _ => None,
        };
        manager
            .sessions
            .insert(tab_id.to_string(), SessionEntry { metadata, writer });
        drop(manager);
        emit_status(app, self);
        Ok(())
    }

    fn record_bytes(&self, app: &AppHandle, tab_id: &str, direction: &str, data: &[u8]) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let writer = match self.manager.lock() {
            Ok(manager) => manager
                .sessions
                .get(tab_id)
                .and_then(|entry| entry.writer.clone()),
            Err(_) => {
                emit_error(app, tab_id, "Terminal log state is unavailable");
                return;
            }
        };
        let Some(writer) = writer else {
            return;
        };
        let result = lock_session_writer(&writer).record_bytes(direction, data);
        if let Err(err) = result {
            self.set_error(err.clone());
            emit_error(app, tab_id, &err);
        }
    }

    fn record_event(&self, app: &AppHandle, tab_id: &str, event: &str, detail: serde_json::Value) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let writer = match self.manager.lock() {
            Ok(manager) => manager
                .sessions
                .get(tab_id)
                .and_then(|entry| entry.writer.clone()),
            Err(_) => {
                emit_error(app, tab_id, "Terminal log state is unavailable");
                return;
            }
        };
        let Some(writer) = writer else {
            return;
        };
        let result = lock_session_writer(&writer).record_event(event, detail);
        if let Err(err) = result {
            self.set_error(err.clone());
            emit_error(app, tab_id, &err);
        }
    }

    fn end_session(&self, app: &AppHandle, tab_id: &str) {
        let writer = self
            .manager
            .lock()
            .ok()
            .and_then(|mut manager| manager.sessions.remove(tab_id))
            .and_then(|entry| entry.writer);
        if let Some(writer) = writer {
            if let Err(err) = lock_session_writer(&writer).finish() {
                emit_error(app, tab_id, &err);
            }
        }
        emit_status(app, self);
    }

    fn status(&self) -> Result<TerminalLogStatus, String> {
        let (config, active_sessions, last_error) = {
            let manager = self
                .manager
                .lock()
                .map_err(|_| "Terminal log state is unavailable")?;
            let config = manager.config.clone().unwrap_or(LogConfig {
                enabled: false,
                directory: crate::config::get_config_path()?.join("logs"),
                format: LOG_FORMAT_BOTH.to_string(),
                name_template: String::new(),
                max_file_size_bytes: 50 * 1024 * 1024,
                compress: false,
            });
            let active_sessions = manager
                .sessions
                .values()
                .filter(|entry| entry.writer.is_some())
                .count();
            (config, active_sessions, manager.last_error.clone())
        };
        Ok(TerminalLogStatus {
            enabled: config.enabled,
            directory: config.directory.to_string_lossy().into_owned(),
            active_sessions,
            total_size_bytes: directory_size(&config.directory),
            last_error,
        })
    }
}

pub fn start_session(
    app: &AppHandle,
    tab_id: &str,
    session_nonce: u32,
    plan: &SessionPlan,
) -> Result<(), String> {
    app.state::<SessionLogState>()
        .start_session(app, tab_id, session_nonce, plan)
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
    app.state::<SessionLogState>().record_event(
        app,
        tab_id,
        "resize",
        serde_json::json!({ "rows": rows, "cols": cols }),
    );
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

#[tauri::command]
pub fn open_terminal_log_directory(
    app: AppHandle,
    state: State<'_, SessionLogState>,
) -> Result<(), String> {
    let directory = {
        let manager = state
            .manager
            .lock()
            .map_err(|_| "Terminal log state is unavailable")?;
        manager
            .config
            .as_ref()
            .map(|config| config.directory.clone())
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
        let mut manager = state
            .manager
            .lock()
            .map_err(|_| "Terminal log state is unavailable")?;
        manager.config = None;
    }
    state.apply_config(&app, &config)?;
    state.status()
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

fn render_name(template: &str, metadata: &SessionMetadata) -> String {
    let now = Local::now();
    let rendered = template
        .replace("{profile}", &metadata.profile)
        .replace("{host}", &metadata.host)
        .replace("{port}", &metadata.port.to_string())
        .replace("{username}", &metadata.username)
        .replace("{type}", &metadata.session_type)
        .replace("{date}", &now.format("%Y%m%d").to_string())
        .replace("{time}", &now.format("%H%M%S").to_string())
        .replace(
            "{yyyyMMdd-HHmmss}",
            &now.format("%Y%m%d-%H%M%S").to_string(),
        )
        .replace("{sessionId}", &metadata.tab_id);
    sanitize_file_name(&rendered)
}

fn sanitize_file_name(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    let trimmed = sanitized.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "terminal-session".to_string()
    } else {
        trimmed.chars().take(180).collect()
    }
}

fn unique_base_name(directory: &Path, requested: &str, config: &LogConfig) -> String {
    for index in 0u32.. {
        let candidate = if index == 0 {
            requested.to_string()
        } else {
            format!("{requested}-{index}")
        };
        let raw_exists = config.wants_raw()
            && path_or_compressed_exists(&part_path(directory, &candidate, "tlog", 0));
        let plain_exists = config.wants_plain()
            && path_or_compressed_exists(&part_path(directory, &candidate, "log", 0));
        if !raw_exists && !plain_exists {
            return candidate;
        }
    }
    unreachable!()
}

fn path_or_compressed_exists(path: &Path) -> bool {
    path.exists() || PathBuf::from(format!("{}.gz", path.to_string_lossy())).exists()
}

fn part_path(directory: &Path, base_name: &str, extension: &str, part: u32) -> PathBuf {
    if part == 0 {
        directory.join(format!("{base_name}.{extension}"))
    } else {
        directory.join(format!("{base_name}.{part:03}.{extension}"))
    }
}

fn open_new_file(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|err| format!("Failed to create terminal log '{}': {err}", path.display()))
}

fn with_newline(mut data: Vec<u8>) -> Vec<u8> {
    data.push(b'\n');
    data
}

fn spawn_compression(app: AppHandle, path: PathBuf) {
    std::thread::spawn(move || {
        if let Err(err) = compress_file(&path) {
            let message = format!(
                "Failed to compress terminal log '{}': {err}",
                path.display()
            );
            app.state::<SessionLogState>().set_error(message.clone());
            let _ = app.emit_to(
                tauri::EventTarget::any(),
                "terminal-log-error",
                serde_json::json!({ "tabId": null, "message": message }),
            );
        }
    });
}

fn compress_file(path: &Path) -> Result<(), String> {
    let mut source = File::open(path).map_err(|err| err.to_string())?;
    let target_path = PathBuf::from(format!("{}.gz", path.to_string_lossy()));
    let target = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&target_path)
        .map_err(|err| err.to_string())?;
    let mut encoder = GzEncoder::new(target, Compression::default());
    std::io::copy(&mut source, &mut encoder).map_err(|err| err.to_string())?;
    let target = encoder.finish().map_err(|err| err.to_string())?;
    target.sync_all().map_err(|err| err.to_string())?;
    fs::remove_file(path).map_err(|err| err.to_string())?;
    Ok(())
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
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

fn emit_error(app: &AppHandle, tab_id: &str, message: &str) {
    let _ = app.emit_to(
        tauri::EventTarget::any(),
        "terminal-log-error",
        serde_json::json!({ "tabId": tab_id, "message": message }),
    );
}

fn emit_status(app: &AppHandle, state: &SessionLogState) {
    if let Ok(status) = state.status() {
        let _ = app.emit_to(tauri::EventTarget::any(), "terminal-log-status", status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn sanitizes_cross_platform_file_names() {
        assert_eq!(sanitize_file_name("root@host:22/a*b?"), "root@host_22_a_b_");
        assert_eq!(sanitize_file_name("..."), "terminal-session");
    }

    #[test]
    fn renders_session_name_tokens() {
        let metadata = SessionMetadata {
            tab_id: "tab:1".to_string(),
            session_nonce: 1,
            session_type: "ssh".to_string(),
            profile: "prod/api".to_string(),
            host: "10.0.0.1".to_string(),
            port: 22,
            username: "root".to_string(),
            started_at_ms: 0,
        };
        let rendered = render_name(
            "{profile}-{host}-{port}-{username}-{type}-{sessionId}",
            &metadata,
        );
        assert_eq!(rendered, "prod_api-10.0.0.1-22-root-ssh-tab_1");
    }

    #[test]
    fn compression_preserves_content() {
        let directory =
            std::env::temp_dir().join(format!("tterm-log-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("session.log");
        fs::write(&source_path, b"terminal output").unwrap();
        compress_file(&source_path).unwrap();
        assert!(!source_path.exists());

        let mut decoder =
            flate2::read::GzDecoder::new(File::open(directory.join("session.log.gz")).unwrap());
        let mut content = String::new();
        decoder.read_to_string(&mut content).unwrap();
        assert_eq!(content, "terminal output");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn compressed_file_prevents_name_reuse() {
        let directory =
            std::env::temp_dir().join(format!("tterm-log-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("session.tlog");
        fs::write(directory.join("session.tlog.gz"), b"archive").unwrap();

        assert!(path_or_compressed_exists(&path));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn plain_output_reconstructs_terminal_redraws() {
        let mut stream = PlainStream::new(false);

        assert!(stream
            .advance(b"\r\x1b[0m\x1b[32m> \x1b[36m~\x1b[0m \x1b[K")
            .is_empty());
        assert!(stream.advance(b"l").is_empty());
        assert!(stream.advance(b"\x08ll").is_empty());
        assert!(stream
            .advance(b"\x08\x08\x1b[32ml\x1b[32ml\x1b[39m")
            .is_empty());

        assert_eq!(stream.advance(b"\x1b[?1l\x1b>\r\r\n"), vec!["> ~ ll"]);
    }

    #[test]
    fn plain_stream_ignores_ansi_only_chunks() {
        let mut stream = PlainStream::new(false);

        assert!(stream.advance(b"\x1b[?1h\x1b=").is_empty());
        assert!(stream.advance(b"\x1b]2;user@host:~\x07").is_empty());
        assert!(stream.finish().is_empty());
    }

    #[test]
    fn plain_input_combines_keystrokes_into_submitted_commands() {
        let mut stream = PlainStream::new(true);

        assert!(stream.advance(b"fast").is_empty());
        for byte in b"fetch" {
            assert!(stream.advance(&[*byte]).is_empty());
        }
        assert_eq!(stream.advance(b"\r"), vec!["fastfetch"]);
        assert!(stream.advance(b"\n").is_empty());
        assert_eq!(stream.advance(b"\x0c"), vec!["<Ctrl+L>"]);
        assert_eq!(stream.advance(b"\r"), vec!["<Enter>"]);
    }
}
