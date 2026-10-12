//! A log file that moves on to a new part once it is full, and compresses
//! the parts it finishes.

use super::naming::{compressed_path, part_path};
use super::SessionLogState;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Emitter, Manager};

/// Logs being compressed; cleanup leaves them (and their archives) alone.
pub(super) static COMPRESSING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

pub(super) struct RotatingFile {
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
    pub(super) fn create(
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

    pub(super) fn write_event(&mut self, data: &[u8]) -> Result<(), String> {
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

    pub(super) fn finish_current(&mut self) -> Result<(), String> {
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

    pub(super) fn open_path(&self) -> Option<&Path> {
        self.file.as_ref().map(|_| self.path.as_path())
    }
}

impl Drop for RotatingFile {
    fn drop(&mut self) {
        let _ = self.finish_current();
    }
}

fn open_new_file(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|err| format!("Failed to create terminal log '{}': {err}", path.display()))
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

pub(super) fn compress_file(path: &Path) -> Result<(), String> {
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
