mod library;
mod retention;
mod screen;

use crate::config::AppConfig;
use crate::core::blocking::run_blocking;
use crate::core::session::SessionPlan;
use crate::core::state::SessionKind;
use base64::Engine;
use chrono::{DateTime, Local};
use flate2::write::GzEncoder;
use flate2::Compression;
use retention::Retention;
use screen::{OutputLog, ScreenLine};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use vte::{Params, Perform};

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

/// Logs being compressed; cleanup leaves them (and their archives) alone.
static COMPRESSING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

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
        self.app
            .state::<SessionLogState>()
            .schedule_cleanup(&self.app, false);
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

    fn open_path(&self) -> Option<&Path> {
        self.file.as_ref().map(|_| self.path.as_path())
    }
}

impl Drop for RotatingFile {
    fn drop(&mut self) {
        let _ = self.finish_current();
    }
}

struct SessionWriter {
    /// The base name of the log's files: its id in the log library.
    id: String,
    started: Instant,
    sequence: u64,
    closed: bool,
    record_input: bool,
    raw: Option<RotatingFile>,
    plain: Option<RotatingFile>,
    plain_lines: PlainLog,
}

/// The plain-text log's lines in the order things happened. An output line
/// is known only once it leaves the screen, so typed lines and events wait
/// until every output line that started before them has been written.
struct PlainLog {
    input: InputStream,
    output: OutputLog,
    queue: VecDeque<(DateTime<Local>, String)>,
    timestamps: bool,
}

impl PlainLog {
    fn new(rows: u16, cols: u16, timestamps: bool) -> Self {
        Self {
            input: InputStream::new(),
            output: OutputLog::new(rows, cols),
            queue: VecDeque::new(),
            timestamps,
        }
    }

    /// `[time] [OUTPUT] text`, or without the time just the text; typed lines
    /// and events keep their tag either way.
    fn line(&self, at: DateTime<Local>, direction: &str, text: &str) -> String {
        match (self.timestamps, direction) {
            (true, _) => format!("[{}] [{direction}] {text}\n", plain_timestamp(at)),
            (false, "OUTPUT") => format!("{text}\n"),
            (false, _) => format!("[{direction}] {text}\n"),
        }
    }

    fn event_line(
        &self,
        at: DateTime<Local>,
        event_type: &str,
        detail: &serde_json::Value,
    ) -> String {
        self.line(at, "EVENT", &format!("{event_type} {detail}"))
    }

    fn input(&mut self, at: DateTime<Local>, data: &[u8]) -> Vec<String> {
        for line in self.input.advance(data) {
            let line = self.line(at, "INPUT", &line);
            self.queue.push_back((at, line));
        }
        self.settle(Vec::new())
    }

    fn output(&mut self, data: &[u8]) -> Vec<String> {
        let lines = self.output.advance(data);
        let ready = self.place(lines);
        self.settle(ready)
    }

    fn event(
        &mut self,
        at: DateTime<Local>,
        event_type: &str,
        detail: &serde_json::Value,
    ) -> Vec<String> {
        let line = self.event_line(at, event_type, detail);
        self.queue.push_back((at, line));
        self.settle(Vec::new())
    }

    fn resize(&mut self, rows: u16, cols: u16) -> Vec<String> {
        let lines = self.output.resize(rows, cols);
        let ready = self.place(lines);
        self.settle(ready)
    }

    /// Everything left: the line being typed and what is still on screen.
    fn finish(&mut self, at: DateTime<Local>) -> Vec<String> {
        for line in self.input.finish() {
            let line = self.line(at, "INPUT", &line);
            self.queue.push_back((at, line));
        }
        let lines = self.output.finish();
        let ready = self.place(lines);
        self.settle(ready)
    }

    /// Takes queued lines from the front while `due` says so, or while the
    /// queue is over its limit.
    fn take_queued(&mut self, ready: &mut Vec<String>, due: impl Fn(DateTime<Local>) -> bool) {
        while let Some((at, _)) = self.queue.front() {
            if !due(*at) && self.queue.len() <= MAX_QUEUED_PLAIN_LINES {
                break;
            }
            let (_, line) = self.queue.pop_front().expect("front checked");
            ready.push(line);
        }
    }

    /// Output lines in order, each after the queued lines older than it.
    fn place(&mut self, lines: Vec<ScreenLine>) -> Vec<String> {
        let mut ready = Vec::new();
        for line in lines {
            match line {
                ScreenLine::Text { at, text } => {
                    self.take_queued(&mut ready, |queued| queued <= at);
                    ready.push(if text.is_empty() {
                        "\n".to_string()
                    } else {
                        self.line(at, "OUTPUT", &text)
                    });
                }
                ScreenLine::Event { at, name } => {
                    let line = self.event_line(at, name, &serde_json::json!({}));
                    self.queue.push_back((at, line));
                }
            }
        }
        ready
    }

    /// Adds the queued lines that no output line still on screen can precede.
    fn settle(&mut self, mut ready: Vec<String>) -> Vec<String> {
        match self.output.oldest_pending() {
            Some(oldest) => self.take_queued(&mut ready, |queued| queued < oldest),
            None => self.take_queued(&mut ready, |_| true),
        }
        ready
    }
}

/// The submitted lines in keyboard input.
struct InputStream {
    parser: vte::Parser,
    line: InputLine,
}

impl InputStream {
    fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
            line: InputLine::default(),
        }
    }

    fn advance(&mut self, data: &[u8]) -> Vec<String> {
        self.parser.advance(&mut self.line, data);
        std::mem::take(&mut self.line.completed)
    }

    fn finish(&mut self) -> Vec<String> {
        if self
            .line
            .text
            .iter()
            .any(|character| !character.is_whitespace())
        {
            self.line.complete();
        }
        std::mem::take(&mut self.line.completed)
    }
}

/// The line being typed, edited by the keys that edit it.
#[derive(Default)]
struct InputLine {
    text: Vec<char>,
    cursor: usize,
    completed: Vec<String>,
    just_committed_cr: bool,
}

impl InputLine {
    fn complete(&mut self) {
        let line = self.text.iter().collect::<String>();
        self.completed.push(line.trim_end().to_string());
        self.text.clear();
        self.cursor = 0;
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

impl InputLine {
    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            if self.cursor < self.text.len() {
                self.text.remove(self.cursor);
            }
        }
    }
}

impl Perform for InputLine {
    fn print(&mut self, character: char) {
        self.just_committed_cr = false;
        // The parser hands DEL, the usual Backspace key, over as printable.
        if character == '\x7f' {
            self.backspace();
            return;
        }
        if self.cursor < self.text.len() {
            self.text[self.cursor] = character;
        } else {
            self.text.resize(self.cursor, ' ');
            self.text.push(character);
        }
        self.cursor = self.cursor.saturating_add(1);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' if self.just_committed_cr => self.just_committed_cr = false,
            b'\n' => self.complete(),
            b'\r' => {
                if self.text.is_empty() {
                    self.completed.push("<Enter>".to_string());
                } else {
                    self.complete();
                }
                self.just_committed_cr = true;
            }
            0x08 => self.backspace(),
            0x03 => {
                if !self.text.is_empty() {
                    self.complete();
                }
                self.completed.push("<Ctrl+C>".to_string());
            }
            0x04 => self.completed.push("<Ctrl+D>".to_string()),
            0x0c => self.completed.push("<Ctrl+L>".to_string()),
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
            'C' => {
                self.cursor = (self.cursor + Self::first_param(params, 1)).min(self.text.len());
            }
            'D' => self.cursor = self.cursor.saturating_sub(Self::first_param(params, 1)),
            'H' => self.cursor = 0,
            'F' => self.cursor = self.text.len(),
            // Delete.
            '~' if Self::first_param(params, 0) == 3 => {
                if self.cursor < self.text.len() {
                    self.text.remove(self.cursor);
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
        rows: u16,
        cols: u16,
    ) -> Result<Self, String> {
        let files = &config.files;
        fs::create_dir_all(&files.directory).map_err(|err| {
            format!(
                "Failed to create terminal log directory '{}': {err}",
                files.directory.display()
            )
        })?;
        let rendered_name = render_name(&files.name_template, &metadata);
        let base_name = unique_base_name(&files.directory, &rendered_name, files);
        if let Some(folder) = library::join_relative(&files.directory, &base_name).parent() {
            fs::create_dir_all(folder).map_err(|err| {
                format!(
                    "Failed to create terminal log folder '{}': {err}",
                    folder.display()
                )
            })?;
        }
        let raw_header = with_newline(
            serde_json::to_vec(&serde_json::json!({
                "type": "header",
                "version": 1,
                "metadata": &metadata,
            }))
            .map_err(|err| format!("Failed to serialize terminal log header: {err}"))?,
        );
        let plain_header = format!(
            "{}\n# type={} profile={} host={} port={} username={} startedAt={} tab={}\n",
            library::PLAIN_HEADER,
            metadata.session_type,
            metadata.profile,
            metadata.host,
            metadata.port,
            metadata.username,
            metadata.started_at_ms,
            metadata.tab_id
        );
        let raw = if files.wants_raw() {
            Some(RotatingFile::create(
                app,
                &files.directory,
                &base_name,
                "tlog",
                files.max_file_size_bytes,
                files.compress,
                &raw_header,
            )?)
        } else {
            None
        };
        let plain = if files.wants_plain() {
            Some(RotatingFile::create(
                app,
                &files.directory,
                &base_name,
                "log",
                files.max_file_size_bytes,
                files.compress,
                plain_header.as_bytes(),
            )?)
        } else {
            None
        };

        Ok(Self {
            id: base_name,
            started: Instant::now(),
            sequence: 0,
            closed: false,
            record_input: config.record_input,
            raw,
            plain,
            plain_lines: PlainLog::new(rows, cols, files.plain_timestamps),
        })
    }

    fn record_bytes(&mut self, direction: &str, data: &[u8]) -> Result<(), String> {
        if self.closed || data.is_empty() {
            return Ok(());
        }
        let is_input = direction == "input";
        if is_input && !self.record_input {
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
        if self.plain.is_some() {
            let lines = if is_input {
                self.plain_lines.input(Local::now(), data)
            } else {
                self.plain_lines.output(data)
            };
            self.write_plain(lines)?;
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
        if self.plain.is_some() {
            let lines = self.plain_lines.event(Local::now(), event_type, &detail);
            self.write_plain(lines)?;
        }
        Ok(())
    }

    fn resize(&mut self, rows: u16, cols: u16) -> Result<(), String> {
        self.record_event("resize", serde_json::json!({ "rows": rows, "cols": cols }))?;
        if self.plain.is_some() && !self.closed {
            let lines = self.plain_lines.resize(rows, cols);
            self.write_plain(lines)?;
        }
        Ok(())
    }

    fn write_plain(&mut self, lines: Vec<String>) -> Result<(), String> {
        let Some(plain) = self.plain.as_mut() else {
            return Ok(());
        };
        for line in lines {
            plain.write_event(line.as_bytes())?;
        }
        Ok(())
    }

    fn close_with(&mut self, event_type: &str) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        if self.plain.is_some() {
            let lines = self.plain_lines.finish(Local::now());
            self.write_plain(lines)?;
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

    fn open_paths(&self) -> impl Iterator<Item = PathBuf> + '_ {
        [self.raw.as_ref(), self.plain.as_ref()]
            .into_iter()
            .flatten()
            .filter_map(|file| file.open_path().map(Path::to_path_buf))
    }
}

fn plain_timestamp(at: DateTime<Local>) -> impl std::fmt::Display {
    at.format("%Y-%m-%d %H:%M:%S%.3f")
}

struct SessionEntry {
    metadata: SessionMetadata,
    policy: SessionLogPolicy,
    rows: u16,
    cols: u16,
    writer: Option<Arc<Mutex<SessionWriter>>>,
}

#[derive(Default)]
struct LogManager {
    config: Option<LogConfig>,
    sessions: HashMap<String, SessionEntry>,
    /// Tabs the user started or stopped logging for by hand; the choice
    /// outlasts a reconnect.
    manual: HashMap<String, bool>,
    /// The logs each tab has written since tTerm started, oldest first. Tab
    /// ids are reused across runs, so this is what ties a tab to its logs.
    logged: HashMap<String, Vec<String>>,
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

fn lock_session_writer(writer: &Arc<Mutex<SessionWriter>>) -> MutexGuard<'_, SessionWriter> {
    match writer.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
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

impl SessionLogState {
    fn set_error(&self, message: String) {
        if let Ok(mut manager) = self.manager.lock() {
            manager.last_error = Some(message);
        }
    }

    fn lock_manager(&self) -> Result<MutexGuard<'_, LogManager>, String> {
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
    fn start_session(
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

    fn record_bytes(&self, app: &AppHandle, tab_id: &str, direction: &str, data: &[u8]) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }
        let Some(writer) = self.writer(app, tab_id) else {
            return;
        };
        let result = lock_session_writer(&writer).record_bytes(direction, data);
        self.report(app, tab_id, result);
    }

    fn record_event(&self, app: &AppHandle, tab_id: &str, event: &str, detail: serde_json::Value) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }
        let Some(writer) = self.writer(app, tab_id) else {
            return;
        };
        let result = lock_session_writer(&writer).record_event(event, detail);
        self.report(app, tab_id, result);
    }

    fn resize(&self, app: &AppHandle, tab_id: &str, rows: u16, cols: u16) {
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

    fn set_recording(
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

    fn end_session(&self, app: &AppHandle, tab_id: &str) {
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

    fn status(&self) -> Result<TerminalLogStatus, String> {
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
    fn schedule_cleanup(&self, app: &AppHandle, force: bool) {
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
    fn directory_state(&self) -> Result<(PathBuf, Retention, HashSet<PathBuf>), String> {
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

/// The template's parts: folders, then the file name. `/` and `\\` both
/// separate them; empty parts are dropped.
fn template_parts(template: &str) -> impl Iterator<Item = &str> {
    template
        .split(['/', '\\'])
        .filter(|part| !part.trim().is_empty())
}

/// The base name of a session's files, relative to the log directory: the
/// template's folders and file name with their variables filled in, joined
/// by `/`. A folder that comes out empty is left out.
fn render_name(template: &str, metadata: &SessionMetadata) -> String {
    let now = Local::now();
    let render = |part: &str| {
        part.replace("{profile}", &metadata.profile)
            .replace("{host}", &metadata.host)
            .replace("{port}", &metadata.port.to_string())
            .replace("{username}", &metadata.username)
            .replace("{type}", &metadata.session_type)
            .replace("{year}", &now.format("%Y").to_string())
            .replace("{month}", &now.format("%m").to_string())
            .replace("{day}", &now.format("%d").to_string())
            .replace("{date}", &now.format("%Y%m%d").to_string())
            .replace("{time}", &now.format("%H%M%S").to_string())
            .replace(
                "{yyyyMMdd-HHmmss}",
                &now.format("%Y%m%d-%H%M%S").to_string(),
            )
            .replace("{sessionId}", &metadata.tab_id)
    };
    let mut parts: Vec<&str> = template_parts(template).collect();
    let file = parts.pop().unwrap_or_default();
    let mut rendered: Vec<String> = parts
        .into_iter()
        .filter_map(|folder| {
            let name = sanitize_name_part(&render(folder));
            (!name.is_empty()).then_some(name)
        })
        .collect();
    rendered.push(sanitize_file_name(&render(file)));
    rendered.join("/")
}

fn sanitize_file_name(value: &str) -> String {
    let name = sanitize_name_part(value);
    if name.is_empty() {
        "terminal-session".to_string()
    } else {
        name
    }
}

/// `value` as one file or folder name on every platform; empty if nothing is
/// left of it.
fn sanitize_name_part(value: &str) -> String {
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
    sanitized
        .trim()
        .trim_matches('.')
        .trim()
        .chars()
        .take(180)
        .collect()
}

fn unique_base_name(directory: &Path, requested: &str, files: &LogFiles) -> String {
    for index in 0u32.. {
        let candidate = if index == 0 {
            requested.to_string()
        } else {
            format!("{requested}-{index}")
        };
        let raw_exists = files.wants_raw()
            && path_or_compressed_exists(&part_path(directory, &candidate, "tlog", 0));
        let plain_exists = files.wants_plain()
            && path_or_compressed_exists(&part_path(directory, &candidate, "log", 0));
        if !raw_exists && !plain_exists {
            return candidate;
        }
    }
    unreachable!()
}

fn path_or_compressed_exists(path: &Path) -> bool {
    path.exists() || compressed_path(path).exists()
}

fn compressed_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.gz", path.to_string_lossy()))
}

/// A log file's path; `base_name` may start with folders, joined by `/`.
fn part_path(directory: &Path, base_name: &str, extension: &str, part: u32) -> PathBuf {
    if part == 0 {
        library::join_relative(directory, &format!("{base_name}.{extension}"))
    } else {
        library::join_relative(directory, &format!("{base_name}.{part:03}.{extension}"))
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
    let held = [path.clone(), compressed_path(&path)];
    if let Ok(mut compressing) = COMPRESSING.lock() {
        compressing.extend(held.iter().cloned());
    }
    std::thread::spawn(move || {
        let result = compress_file(&path);
        if let Ok(mut compressing) = COMPRESSING.lock() {
            for path in &held {
                compressing.remove(path);
            }
        }
        if let Err(err) = result {
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
    let target_path = compressed_path(path);
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
    fn renders_folders_from_the_template() {
        let metadata = SessionMetadata {
            tab_id: "tab-1".to_string(),
            session_nonce: 1,
            session_type: "ssh".to_string(),
            profile: "prod/api".to_string(),
            host: "10.0.0.1".to_string(),
            port: 22,
            username: String::new(),
            started_at_ms: 0,
        };
        let year = Local::now().format("%Y").to_string();
        assert_eq!(
            render_name("{year}/{profile}/{host}-{sessionId}", &metadata),
            format!("{year}/prod_api/10.0.0.1-tab-1")
        );
        // Backslashes separate folders too; empty and dot-only folders are dropped.
        assert_eq!(
            render_name("/logs\\{username}//../{host}", &metadata),
            "logs/10.0.0.1"
        );
        assert_eq!(render_name("{type}/", &metadata), "ssh");
    }

    #[test]
    fn limits_how_deep_the_template_goes() {
        let mut config = AppConfig {
            terminal_log_directory: std::env::temp_dir().to_string_lossy().into_owned(),
            ..AppConfig::default()
        };
        config.terminal_log_name_template = "a/b/c/d/e/{host}".to_string();
        assert_eq!(LogConfig::from_app_config(&config).err(), None);
        config.terminal_log_name_template = "a/b/c/d/e/f/{host}".to_string();
        assert!(LogConfig::from_app_config(&config).is_err());
    }

    #[test]
    fn part_paths_follow_folders() {
        let directory = Path::new("/logs");
        assert_eq!(
            part_path(directory, "2026/prod-1", "tlog", 2),
            directory.join("2026").join("prod-1.002.tlog")
        );
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
    fn input_combines_keystrokes_into_submitted_commands() {
        let mut stream = InputStream::new();

        assert!(stream.advance(b"fast").is_empty());
        for byte in b"fetch" {
            assert!(stream.advance(&[*byte]).is_empty());
        }
        assert_eq!(stream.advance(b"\r"), vec!["fastfetch"]);
        assert!(stream.advance(b"\n").is_empty());
        assert_eq!(stream.advance(b"\x0c"), vec!["<Ctrl+L>"]);
        assert_eq!(stream.advance(b"\r"), vec!["<Enter>"]);
    }

    #[test]
    fn input_follows_line_editing_keys() {
        let mut stream = InputStream::new();
        // Type "lss", move left, delete the extra "s", go home, insert nothing.
        assert!(stream.advance(b"lss\x1b[D\x1b[3~\x1b[H").is_empty());
        assert_eq!(stream.advance(b"\r"), vec!["ls"]);
        assert!(stream.advance(b"pwdx\x7f").is_empty());
        assert_eq!(stream.finish(), vec!["pwd"]);
    }

    /// The log lines without their timestamps.
    fn untimed(lines: Vec<String>) -> Vec<String> {
        lines
            .into_iter()
            .map(|line| match line.split_once("] ") {
                Some((_, rest)) => rest.trim_end().replacen("[", "", 1).replacen("]", "", 1),
                None => line.trim_end().to_string(),
            })
            .collect()
    }

    fn tick() -> DateTime<Local> {
        std::thread::sleep(Duration::from_millis(2));
        Local::now()
    }

    #[test]
    fn plain_log_puts_typed_lines_between_the_output_around_them() {
        let mut log = PlainLog::new(10, 40, true);
        let mut lines = log.output(b"$ ");
        tick();
        lines.extend(log.input(tick(), b"ls\r"));
        // The prompt is still on screen, so the typed line waits for it.
        assert!(lines.is_empty());
        tick();
        lines.extend(log.output(b"ls\r\nfile\r\n$ "));
        lines.extend(log.finish(tick()));
        assert_eq!(
            untimed(lines),
            vec!["OUTPUT $ ls", "INPUT ls", "OUTPUT file", "OUTPUT $"]
        );
    }

    #[test]
    fn plain_log_without_timestamps_keeps_output_bare() {
        let mut log = PlainLog::new(10, 40, false);
        let mut lines = log.event(tick(), "session_start", &serde_json::json!({}));
        lines.extend(log.output(b"$ "));
        lines.extend(log.input(tick(), b"ls\r"));
        tick();
        lines.extend(log.output(b"ls\r\nfile\r\n\r\n$ "));
        lines.extend(log.finish(tick()));
        assert_eq!(
            lines,
            vec![
                "[EVENT] session_start {}\n",
                "$ ls\n",
                "[INPUT] ls\n",
                "file\n",
                "\n",
                "$\n",
            ]
        );
    }

    #[test]
    fn plain_log_writes_events_right_away_on_an_empty_screen() {
        let mut log = PlainLog::new(10, 40, true);
        let lines = log.event(Local::now(), "session_start", &serde_json::json!({}));
        assert_eq!(untimed(lines), vec!["EVENT session_start {}"]);
    }

    #[test]
    fn plain_log_notes_full_screen_programs_in_order() {
        let mut log = PlainLog::new(10, 40, true);
        let mut lines = log.output(b"$ vim\r\n");
        tick();
        lines.extend(log.output(b"\x1b[?1049h\x1b[2Jtext\x1b[?1049l"));
        tick();
        lines.extend(log.output(b"$ ls\r\n"));
        lines.extend(log.finish(tick()));
        assert_eq!(
            untimed(lines),
            vec![
                "OUTPUT $ vim",
                "EVENT fullscreen_start {}",
                "EVENT fullscreen_end {}",
                "OUTPUT $ ls",
            ]
        );
    }

    #[test]
    fn policy_and_manual_choice_decide_recording() {
        let config = |record_by_default| LogConfig {
            record_by_default,
            record_input: false,
            files: LogFiles {
                directory: PathBuf::new(),
                format: LOG_FORMAT_BOTH.to_string(),
                name_template: "x".to_string(),
                max_file_size_bytes: 1,
                compress: false,
                plain_timestamps: true,
            },
            retention: Retention::default(),
        };
        let off = config(false);
        let on = config(true);

        assert!(!off.records(SessionLogPolicy::Default, None));
        assert!(on.records(SessionLogPolicy::Default, None));
        assert!(off.records(SessionLogPolicy::Always, None));
        assert!(!on.records(SessionLogPolicy::Never, None));
        assert!(off.records(SessionLogPolicy::Never, Some(true)));
        assert!(!on.records(SessionLogPolicy::Always, Some(false)));
    }

    #[test]
    fn policy_reads_profile_labels() {
        assert_eq!(
            SessionLogPolicy::from_label(Some("always")),
            SessionLogPolicy::Always
        );
        assert_eq!(
            SessionLogPolicy::from_label(Some("never")),
            SessionLogPolicy::Never
        );
        assert_eq!(
            SessionLogPolicy::from_label(None),
            SessionLogPolicy::Default
        );
        assert_eq!(
            SessionLogPolicy::from_label(Some("other")),
            SessionLogPolicy::Default
        );
    }
}
