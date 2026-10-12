//! A log file that moves on to a new part once it is full, and compresses
//! the parts it finishes.

use super::naming::{compressed_path, part_path};
use super::SessionLogState;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Emitter, Manager};

/// Logs being written or compressed, and the archives being written; cleanup
/// and deletion leave them alone. A file is added before it is
/// opened and removed only once nothing will touch it again, so there is no
/// moment when a log in use is missing from it.
static IN_USE: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

/// How much a log keeps in memory before writing it out. Its session's
/// writer also writes it out at least once a second.
const BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn in_use() -> HashSet<PathBuf> {
    match IN_USE.lock() {
        Ok(in_use) => in_use.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

fn hold(paths: &[&Path]) {
    let mut in_use = match IN_USE.lock() {
        Ok(in_use) => in_use,
        Err(poisoned) => poisoned.into_inner(),
    };
    in_use.extend(paths.iter().map(|path| path.to_path_buf()));
}

fn release(paths: &[&Path]) {
    let mut in_use = match IN_USE.lock() {
        Ok(in_use) => in_use,
        Err(poisoned) => poisoned.into_inner(),
    };
    for path in paths {
        in_use.remove(*path);
    }
}

pub(super) struct RotatingFile {
    file: Option<BufWriter<File>>,
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
        let file = open_part(&path, header)?;
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

    /// Writes out what is buffered, so readers of the file see it.
    pub(super) fn flush(&mut self) -> Result<(), String> {
        let Some(file) = self.file.as_mut() else {
            return Ok(());
        };
        file.flush().map_err(|err| {
            format!(
                "Failed to write terminal log '{}': {err}",
                self.path.display()
            )
        })
    }

    fn rotate(&mut self) -> Result<(), String> {
        self.finish_current()?;
        self.part = self.part.saturating_add(1);
        self.path = part_path(&self.directory, &self.base_name, self.extension, self.part);
        self.file = Some(open_part(&self.path, &self.header)?);
        self.bytes_written = self.header.len() as u64;
        self.app
            .state::<SessionLogState>()
            .schedule_maintenance(&self.app, false);
        Ok(())
    }

    pub(super) fn finish_current(&mut self) -> Result<(), String> {
        let Some(mut file) = self.file.take() else {
            return Ok(());
        };
        let result = file.flush().and_then(|()| file.get_ref().sync_all());
        drop(file);
        if self.compress && result.is_ok() {
            spawn_compression(self.app.clone(), self.path.clone());
        } else {
            release(&[&self.path]);
        }
        result.map_err(|err| {
            format!(
                "Failed to flush terminal log '{}': {err}",
                self.path.display()
            )
        })
    }
}

impl Drop for RotatingFile {
    fn drop(&mut self) {
        let _ = self.finish_current();
    }
}

/// Creates a part and writes its header, holding it from before it exists.
fn open_part(path: &Path, header: &[u8]) -> Result<BufWriter<File>, String> {
    hold(&[path]);
    let result = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|err| format!("Failed to create terminal log '{}': {err}", path.display()))
        .and_then(|file| {
            let mut file = BufWriter::with_capacity(BUFFER_BYTES, file);
            file.write_all(header)
                .map_err(|err| format!("Failed to write terminal log header: {err}"))?;
            Ok(file)
        });
    if result.is_err() {
        release(&[path]);
    }
    result
}

/// Compresses a finished part on its own thread. The part is already held;
/// its archive is held too until the work is done.
fn spawn_compression(app: AppHandle, path: PathBuf) {
    let archive = compressed_path(&path);
    hold(&[&archive]);
    let held = [path.clone(), archive];
    let spawned = std::thread::Builder::new()
        .name("tterm-log-compress".to_string())
        .spawn({
            let held = held.clone();
            move || {
                let result = compress_file(&path);
                release(&held.each_ref().map(PathBuf::as_path));
                let state = app.state::<SessionLogState>();
                state.schedule_maintenance(&app, false);
                if let Err(err) = result {
                    let message = format!(
                        "Failed to compress terminal log '{}': {err}",
                        path.display()
                    );
                    state.set_error(message.clone());
                    let _ = app.emit_to(
                        tauri::EventTarget::any(),
                        "terminal-log-error",
                        serde_json::json!({ "tabId": null, "message": message }),
                    );
                }
            }
        });
    if spawned.is_err() {
        // The part stays as it is, uncompressed.
        release(&held.each_ref().map(PathBuf::as_path));
    }
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
