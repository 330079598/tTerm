use super::protocol::{self, Link, OutgoingFile, Received, TransferError};
use super::XmodemProtocol;
use crate::core::state::ZmodemMap;
use crate::serial::io::SerialTap;
use crate::terminal::types::PtyWriter;
use crate::zmodem::events::{
    self, EmitFn, TransferCompletePayload, TransferProgressPayload, TransferStartPayload,
};
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// How often the transfer list hears how far a file has got.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// The port as a transfer sees it: what the reader diverts to it, and the
/// port's writer.
pub(super) struct TapLink {
    incoming: Receiver<Vec<u8>>,
    pending: VecDeque<u8>,
    writer: PtyWriter,
    cancel_requested: Arc<AtomicBool>,
}

impl TapLink {
    pub(super) fn new(
        incoming: Receiver<Vec<u8>>,
        writer: PtyWriter,
        cancel_requested: Arc<AtomicBool>,
    ) -> Self {
        Self {
            incoming,
            pending: VecDeque::new(),
            writer,
            cancel_requested,
        }
    }
}

impl Link for TapLink {
    fn read(&mut self, timeout: Duration) -> Received {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(byte) = self.pending.pop_front() {
                return Received::Byte(byte);
            }
            match self
                .incoming
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(bytes) => self.pending.extend(bytes),
                Err(RecvTimeoutError::Timeout) => return Received::TimedOut,
                Err(RecvTimeoutError::Disconnected) => return Received::Closed,
            }
        }
    }

    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes)
    }

    fn cancelled(&self) -> bool {
        self.cancel_requested.load(Ordering::Relaxed)
    }
}

/// A file opened to send, with where it came from for the transfer list.
pub(super) struct OpenedFile {
    file: OutgoingFile<File>,
    path: String,
}

/// Opens the files to send, failing before the transfer starts when one
/// can't be read.
pub(super) fn open_files(paths: &[String]) -> Result<Vec<OpenedFile>, String> {
    paths
        .iter()
        .map(|path| {
            let data = File::open(path).map_err(|err| format!("Failed to open {path}: {err}"))?;
            let metadata = data
                .metadata()
                .map_err(|err| format!("Failed to read {path}: {err}"))?;
            if !metadata.is_file() {
                return Err(format!("{path} is not a file"));
            }
            let name = Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|age| age.as_secs());
            Ok(OpenedFile {
                file: OutgoingFile {
                    name,
                    size: metadata.len(),
                    modified,
                    data,
                },
                path: path.clone(),
            })
        })
        .collect()
}

/// A transfer's hold on a tab's port, given back with [`PortClaim::release`].
pub(super) struct PortClaim {
    pub tab_id: String,
    pub session_nonce: u32,
    pub transfers: ZmodemMap,
    pub tap: SerialTap,
}

impl PortClaim {
    /// Gives the port's input back to the terminal and lets typing through.
    pub(super) fn release(self) {
        *self.tap.lock().unwrap_or_else(PoisonError::into_inner) = None;
        let mut transfers = self
            .transfers
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        if transfers
            .get(&self.tab_id)
            .is_some_and(|handle| handle.session_nonce == self.session_nonce)
        {
            transfers.remove(&self.tab_id);
        }
    }
}

/// Tells the device a transfer that failed on this side is over. One the
/// device cancelled, or that lost the device, needs no telling.
pub(super) fn cancel_on_failure(link: &mut TapLink, result: &Result<(), TransferError>) {
    if let Err(error) = result {
        if !matches!(
            error,
            TransferError::CancelledByDevice | TransferError::Disconnected
        ) {
            protocol::send_cancel(link);
        }
    }
}

pub(super) struct TransferParams {
    pub emit: EmitFn,
    pub claim: PortClaim,
    pub link: TapLink,
    pub protocol: XmodemProtocol,
    pub files: Vec<OpenedFile>,
}

/// What the transfer list shows of a file.
pub(super) struct FileInfo {
    pub name: String,
    /// `None` when the sender gave none.
    pub size: Option<u64>,
    pub path: String,
    /// `upload` to the device, `download` from it.
    pub direction: &'static str,
}

/// Reports the files' rows in the transfer list.
pub(super) struct FileRows<'a> {
    emit: &'a EmitFn,
    files: Vec<FileInfo>,
    ids: Vec<String>,
    started: Vec<Option<Instant>>,
    last_report: Option<Instant>,
}

impl<'a> FileRows<'a> {
    pub(super) fn new(emit: &'a EmitFn, files: Vec<FileInfo>) -> Self {
        Self {
            emit,
            ids: files.iter().map(|_| events::new_transfer_id()).collect(),
            started: vec![None; files.len()],
            files,
            last_report: None,
        }
    }

    /// Adds a file found as the transfer runs; returns its index.
    pub(super) fn add(&mut self, file: FileInfo) -> usize {
        self.files.push(file);
        self.ids.push(events::new_transfer_id());
        self.started.push(None);
        self.files.len() - 1
    }

    pub(super) fn start(&mut self, index: usize) {
        if self.started[index].is_some() {
            return;
        }
        self.started[index] = Some(Instant::now());
        let file = &self.files[index];
        events::emit(
            self.emit,
            "transfer-start",
            TransferStartPayload {
                transfer_id: self.ids[index].clone(),
                direction: file.direction,
                file_name: file.name.clone(),
                file_size: file.size.unwrap_or(0),
                local_path: file.path.clone(),
                size_unknown: file.size.is_none(),
            },
        );
    }

    pub(super) fn progress(&mut self, index: usize, sent: u64, finished: bool) {
        let now = Instant::now();
        let due = self
            .last_report
            .is_none_or(|last| now.duration_since(last) >= PROGRESS_INTERVAL);
        if !due && !finished {
            return;
        }
        self.last_report = Some(now);
        let elapsed = self.started[index]
            .map_or(0.0, |started| now.duration_since(started).as_secs_f64())
            .max(0.001);
        events::emit(
            self.emit,
            "transfer-progress",
            TransferProgressPayload {
                transfer_id: self.ids[index].clone(),
                transferred: sent,
                speed: (sent as f64 / elapsed) as u64,
            },
        );
    }

    pub(super) fn complete(&self, index: usize, result: &Result<(), TransferError>) {
        let cancelled = matches!(result, Err(TransferError::Cancelled));
        events::emit(
            self.emit,
            "transfer-complete",
            TransferCompletePayload {
                transfer_id: self.ids[index].clone(),
                success: result.is_ok(),
                cancelled,
                error: result.as_ref().err().map(TransferError::message),
            },
        );
    }

    /// Ends the rows of the files from `from` up to, not including, `to` as
    /// sent, empty files among them that never reported progress.
    pub(super) fn finish(&mut self, from: usize, to: usize) {
        for index in from..to {
            self.start(index);
            self.progress(index, self.files[index].size.unwrap_or(0), true);
            self.complete(index, &Ok(()));
        }
    }
}

/// Runs a transfer to its end, then gives the port back to the terminal.
pub(super) fn run(params: TransferParams) {
    let TransferParams {
        emit,
        claim,
        mut link,
        protocol,
        files,
    } = params;
    let (infos, mut outgoing): (Vec<_>, Vec<_>) = files
        .into_iter()
        .map(|opened| {
            let info = FileInfo {
                name: opened.file.name.clone(),
                size: Some(opened.file.size),
                path: opened.path,
                direction: "upload",
            };
            (info, opened.file)
        })
        .unzip();
    let count = outgoing.len();
    let mut rows = FileRows::new(&emit, infos);
    // The file the transfer is on, whose row a failure ends.
    let mut current = 0;
    rows.start(0);

    let result = if protocol == XmodemProtocol::Ymodem {
        protocol::send_ymodem(&mut link, &mut outgoing, &mut |index, sent| {
            if index != current {
                rows.finish(current, index);
                current = index;
                rows.start(index);
            }
            rows.progress(index, sent, false);
        })
    } else {
        protocol::send_xmodem(&mut link, protocol, &mut outgoing[0], &mut |sent| {
            rows.progress(0, sent, false);
        })
    };

    cancel_on_failure(&mut link, &result);
    match &result {
        Ok(()) => rows.finish(current, count),
        Err(_) => rows.complete(current, &result),
    }
    claim.release();
}
