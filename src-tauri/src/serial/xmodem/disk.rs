//! Where files received from a serial device are written.

use super::protocol::{Result, TransferError};
use super::receive::{self, FileSink};
use super::transfer::{cancel_on_failure, FileInfo, FileRows, PortClaim, TapLink};
use super::XmodemProtocol;
use crate::zmodem::events::EmitFn;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// The name a file gets when its sender gives none that can be used.
const FALLBACK_NAME: &str = "received.bin";

/// Where a receive saves: XMODEM sends one nameless file, saved where asked;
/// YMODEM names its files, saved in a folder.
pub(super) enum Destination {
    File(PathBuf),
    Folder(PathBuf),
}

impl Destination {
    /// Checks that `destination` can take what `protocol` sends.
    pub(super) fn new(
        protocol: XmodemProtocol,
        destination: &str,
    ) -> std::result::Result<Self, String> {
        let path = PathBuf::from(destination.trim());
        if path.as_os_str().is_empty() {
            return Err("Choose where to save the received files".to_string());
        }
        if protocol == XmodemProtocol::Ymodem {
            return if path.is_dir() {
                Ok(Self::Folder(path))
            } else {
                Err(format!("{} is not a folder", path.display()))
            };
        }
        match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() && !parent.is_dir() => {
                Err(format!("{} is not a folder", parent.display()))
            }
            _ => Ok(Self::File(path)),
        }
    }
}

/// A name the device sent, cut to its last part so it can't leave the
/// folder, with what file systems refuse replaced.
pub(super) fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base
        .chars()
        .map(|char| {
            if char.is_control() || "<>:\"|?*".contains(char) {
                '_'
            } else {
                char
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.');
    if cleaned.is_empty() {
        FALLBACK_NAME.to_string()
    } else {
        cleaned.to_string()
    }
}

/// `folder/name`, or `folder/stem (2).ext` and on when that is taken.
pub(super) fn unused_path(folder: &Path, name: &str) -> PathBuf {
    let path = folder.join(name);
    if !path.exists() {
        return path;
    }
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    (2..)
        .map(|number| folder.join(format!("{stem} ({number}){extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(path)
}

/// The file being written.
struct OpenFile {
    writer: BufWriter<File>,
    path: PathBuf,
    row: usize,
    written: u64,
}

/// Writes received files to disk and shows them in the transfer list.
struct DiskSink<'a> {
    destination: Destination,
    rows: FileRows<'a>,
    open: Option<OpenFile>,
    received: usize,
}

fn io_error(path: &Path, err: std::io::Error) -> TransferError {
    TransferError::Io(format!("Failed to write {}: {err}", path.display()))
}

impl FileSink for DiskSink<'_> {
    fn open(&mut self, name: Option<&str>, size: Option<u64>) -> Result<()> {
        let path = match &self.destination {
            Destination::File(path) => path.clone(),
            Destination::Folder(folder) => {
                unused_path(folder, &safe_file_name(name.unwrap_or(FALLBACK_NAME)))
            }
        };
        let file = File::create(&path).map_err(|err| io_error(&path, err))?;
        let row = self.rows.add(FileInfo {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            size,
            path: path.to_string_lossy().into_owned(),
            direction: "download",
        });
        self.rows.start(row);
        self.open = Some(OpenFile {
            writer: BufWriter::new(file),
            path,
            row,
            written: 0,
        });
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        let Some(open) = self.open.as_mut() else {
            return Ok(());
        };
        open.writer
            .write_all(data)
            .map_err(|err| io_error(&open.path, err))?;
        open.written += data.len() as u64;
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        let Some(mut open) = self.open.take() else {
            return Ok(());
        };
        open.writer
            .flush()
            .map_err(|err| io_error(&open.path, err))?;
        self.rows.progress(open.row, open.written, true);
        self.rows.complete(open.row, &Ok(()));
        self.received += 1;
        Ok(())
    }

    fn progress(&mut self, received: u64) {
        if let Some(open) = &self.open {
            self.rows.progress(open.row, received, false);
        }
    }
}

impl DiskSink<'_> {
    /// Ends a receive that failed: the file it was on is deleted, half
    /// written, and its row shows why. A receive that failed before any file
    /// began gets a row of its own, so the failure shows.
    fn fail(mut self, result: &Result<()>) {
        if let Some(open) = self.open.take() {
            let OpenFile {
                writer, path, row, ..
            } = open;
            drop(writer);
            let _ = std::fs::remove_file(&path);
            self.rows.complete(row, result);
            return;
        }
        if self.received > 0 {
            return;
        }
        let path = match &self.destination {
            Destination::File(path) | Destination::Folder(path) => path,
        };
        let row = self.rows.add(FileInfo {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            size: None,
            path: path.to_string_lossy().into_owned(),
            direction: "download",
        });
        self.rows.start(row);
        self.rows.complete(row, result);
    }
}

/// Receives over `protocol` into `destination`, then gives the port back.
pub(super) fn run_receive(
    emit: EmitFn,
    claim: PortClaim,
    mut link: TapLink,
    protocol: XmodemProtocol,
    destination: Destination,
) {
    let mut sink = DiskSink {
        destination,
        rows: FileRows::new(&emit, Vec::new()),
        open: None,
        received: 0,
    };
    let result = if protocol == XmodemProtocol::Ymodem {
        receive::receive_ymodem(&mut link, &mut sink)
    } else {
        receive::receive_xmodem(&mut link, &mut sink)
    };
    cancel_on_failure(&mut link, &result);
    if result.is_err() {
        sink.fail(&result);
    }
    claim.release();
}
