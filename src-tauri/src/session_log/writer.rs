//! One session's log: its raw and plain-text files.

use super::naming::{render_name, unique_base_name};
use super::plain::PlainLog;
use super::rotating::RotatingFile;
use super::{library, LogConfig, SessionMetadata};
use base64::Engine;
use chrono::{DateTime, Local};
use std::fs;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

/// When something happened, taken as it happened: the log is written later,
/// on its own thread.
#[derive(Clone, Copy, Debug)]
pub(super) struct Stamp {
    instant: Instant,
    wall: SystemTime,
}

impl Stamp {
    pub(super) fn now() -> Self {
        Self {
            instant: Instant::now(),
            wall: SystemTime::now(),
        }
    }

    fn unix_ms(self) -> u64 {
        self.wall
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64
    }

    fn local(self) -> DateTime<Local> {
        DateTime::from(self.wall)
    }
}

pub(super) struct SessionWriter {
    /// The base name of the log's files: its id in the log library.
    pub(super) id: String,
    started: Instant,
    sequence: u64,
    closed: bool,
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
            raw,
            plain,
            plain_lines: PlainLog::new(rows, cols, files.plain_timestamps),
        })
    }

    fn elapsed_us(&self, at: Stamp) -> u64 {
        at.instant
            .saturating_duration_since(self.started)
            .as_micros()
            .min(u64::MAX as u128) as u64
    }

    pub(super) fn record_bytes(
        &mut self,
        direction: &str,
        data: &[u8],
        at: Stamp,
    ) -> Result<(), String> {
        if self.closed || data.is_empty() {
            return Ok(());
        }
        let is_input = direction == "input";
        self.sequence = self.sequence.saturating_add(1);
        let elapsed_us = self.elapsed_us(at);
        if let Some(raw) = self.raw.as_mut() {
            let event = serde_json::to_vec(&serde_json::json!({
                "type": "data",
                "sequence": self.sequence,
                "timestampMs": at.unix_ms(),
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
                self.plain_lines.input(at.local(), data)
            } else {
                self.plain_lines.output(at.local(), data)
            };
            self.write_plain(lines)?;
        }
        Ok(())
    }

    pub(super) fn record_event(
        &mut self,
        event_type: &str,
        detail: serde_json::Value,
        at: Stamp,
    ) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        self.sequence = self.sequence.saturating_add(1);
        let elapsed_us = self.elapsed_us(at);
        if let Some(raw) = self.raw.as_mut() {
            let event = serde_json::to_vec(&serde_json::json!({
                "type": event_type,
                "sequence": self.sequence,
                "timestampMs": at.unix_ms(),
                "elapsedUs": elapsed_us,
                "detail": detail,
            }))
            .map_err(|err| format!("Failed to serialize terminal log event: {err}"))?;
            raw.write_event(&with_newline(event))?;
        }
        if self.plain.is_some() {
            let lines = self.plain_lines.event(at.local(), event_type, &detail);
            self.write_plain(lines)?;
        }
        Ok(())
    }

    pub(super) fn resize(&mut self, rows: u16, cols: u16, at: Stamp) -> Result<(), String> {
        self.record_event(
            "resize",
            serde_json::json!({ "rows": rows, "cols": cols }),
            at,
        )?;
        if self.plain.is_some() && !self.closed {
            let lines = self.plain_lines.resize(at.local(), rows, cols);
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

    /// Writes out what is buffered, so readers of the files see it.
    pub(super) fn flush(&mut self) -> Result<(), String> {
        for file in [self.raw.as_mut(), self.plain.as_mut()]
            .into_iter()
            .flatten()
        {
            file.flush()?;
        }
        Ok(())
    }

    pub(super) fn close_with(&mut self, event_type: &str, at: Stamp) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        if self.plain.is_some() {
            let lines = self.plain_lines.finish(at.local());
            self.write_plain(lines)?;
        }
        self.record_event(event_type, serde_json::json!({}), at)?;
        self.closed = true;
        if let Some(mut raw) = self.raw.take() {
            raw.finish_current()?;
        }
        if let Some(mut plain) = self.plain.take() {
            plain.finish_current()?;
        }
        Ok(())
    }
}

fn with_newline(mut data: Vec<u8>) -> Vec<u8> {
    data.push(b'\n');
    data
}
