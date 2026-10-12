//! One session's log: its raw and plain-text files.

use super::naming::{render_name, unique_base_name};
use super::plain::PlainLog;
use super::rotating::RotatingFile;
use super::{library, now_unix_ms, LogConfig, SessionMetadata};
use base64::Engine;
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;
use tauri::AppHandle;

pub(super) struct SessionWriter {
    /// The base name of the log's files: its id in the log library.
    pub(super) id: String,
    started: Instant,
    sequence: u64,
    closed: bool,
    pub(super) record_input: bool,
    raw: Option<RotatingFile>,
    plain: Option<RotatingFile>,
    plain_lines: PlainLog,
}

impl SessionWriter {
    pub(super) fn create(
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

    pub(super) fn record_bytes(&mut self, direction: &str, data: &[u8]) -> Result<(), String> {
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

    pub(super) fn record_event(
        &mut self,
        event_type: &str,
        detail: serde_json::Value,
    ) -> Result<(), String> {
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

    pub(super) fn resize(&mut self, rows: u16, cols: u16) -> Result<(), String> {
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

    pub(super) fn close_with(&mut self, event_type: &str) -> Result<(), String> {
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

    pub(super) fn finish(&mut self) -> Result<(), String> {
        self.close_with("session_end")
    }

    pub(super) fn open_paths(&self) -> impl Iterator<Item = PathBuf> + '_ {
        [self.raw.as_ref(), self.plain.as_ref()]
            .into_iter()
            .flatten()
            .filter_map(|file| file.open_path().map(Path::to_path_buf))
    }
}

pub(super) fn lock_session_writer(
    writer: &Arc<Mutex<SessionWriter>>,
) -> MutexGuard<'_, SessionWriter> {
    match writer.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn with_newline(mut data: Vec<u8>) -> Vec<u8> {
    data.push(b'\n');
    data
}
