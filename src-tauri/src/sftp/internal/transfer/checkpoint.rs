//! The checkpoint sidecar: where it is stored, and the background writer
//! that persists it off the transfer lanes.

use super::files::{remote_read_all, remote_write_all};
use super::{
    failed, TransferError, TransferSidecar, CHECKPOINT_MIN_BYTES, CHECKPOINT_MIN_INTERVAL,
    CHECKPOINT_QUEUE_CAPACITY,
};
use russh_sftp::client::RawSftpSession;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};

pub(super) enum SidecarStore {
    Remote {
        session: Arc<RawSftpSession>,
        path: String,
        step: u64,
    },
    Local {
        path: PathBuf,
    },
}

impl SidecarStore {
    pub(super) async fn load(&self) -> Option<TransferSidecar> {
        match self {
            SidecarStore::Remote {
                session,
                path,
                step,
            } => {
                if let Some(data) = remote_read_all(session.as_ref(), path, *step)
                    .await
                    .ok()
                    .flatten()
                {
                    if let Ok(sidecar) = serde_json::from_slice::<TransferSidecar>(&data) {
                        return Some(sidecar);
                    }
                }
                // The save swap (write tmp -> remove primary -> rename) can
                // crash after the remove but before the rename, leaving only
                // the temp copy. Recover it instead of discarding the whole
                // transfer's checkpoint.
                let tmp_path = format!("{path}.tmp");
                let data = remote_read_all(session.as_ref(), &tmp_path, *step)
                    .await
                    .ok()
                    .flatten()?;
                let sidecar: TransferSidecar = serde_json::from_slice(&data).ok()?;
                // Best-effort promotion so the next save cycle swaps over a
                // clean primary; harmless when the primary still exists (the
                // SFTPv3 rename then fails and the next save repairs it).
                let _ = session.rename(&tmp_path, path).await;
                Some(sidecar)
            }
            SidecarStore::Local { path } => {
                if let Ok(data) = tokio::fs::read(path).await {
                    if let Ok(sidecar) = serde_json::from_slice::<TransferSidecar>(&data) {
                        return Some(sidecar);
                    }
                }
                // Same recovery as the remote store for the failed
                // remove-then-rename fallback in `save`.
                let tmp = path.with_extension("json.tmp");
                let data = tokio::fs::read(&tmp).await.ok()?;
                let sidecar: TransferSidecar = serde_json::from_slice(&data).ok()?;
                let _ = tokio::fs::rename(&tmp, path).await;
                Some(sidecar)
            }
        }
    }

    pub(super) async fn save(&self, sidecar: &TransferSidecar) -> Result<(), TransferError> {
        let data = serde_json::to_vec(sidecar)
            .map_err(|err| failed(format!("Failed to encode transfer checkpoint: {err}")))?;
        match self {
            SidecarStore::Remote {
                session,
                path,
                step,
            } => {
                // Write-then-swap: a crash mid-write leaves the previous
                // checkpoint intact instead of a truncated one. SFTPv3 rename
                // cannot overwrite, so remove first (leaving a missing — not
                // corrupt — checkpoint for at most one round trip) and fall
                // back to a direct write if the swap still fails.
                let tmp_path = format!("{path}.tmp");
                remote_write_all(session.as_ref(), &tmp_path, &data, *step).await?;
                let _ = session.remove(path).await;
                if session.rename(&tmp_path, path).await.is_err() {
                    remote_write_all(session.as_ref(), path, &data, *step).await?;
                }
                Ok(())
            }
            SidecarStore::Local { path } => {
                let tmp = path.with_extension("json.tmp");
                tokio::fs::write(&tmp, &data)
                    .await
                    .map_err(|err| failed(format!("Failed to write transfer checkpoint: {err}")))?;
                if tokio::fs::rename(&tmp, path).await.is_err() {
                    let _ = tokio::fs::remove_file(path).await;
                    tokio::fs::rename(&tmp, path)
                        .await
                        .map_err(|err| failed(format!("Failed to persist checkpoint: {err}")))?;
                }
                Ok(())
            }
        }
    }

    pub(super) async fn remove(&self) {
        match self {
            SidecarStore::Remote { session, path, .. } => {
                let _ = session.remove(path).await;
                let _ = session.remove(format!("{path}.tmp")).await;
            }
            SidecarStore::Local { path } => {
                let _ = tokio::fs::remove_file(path).await;
                // `save` swaps through a temp copy; a crash between its write
                // and rename would otherwise leave that copy behind forever.
                let _ = tokio::fs::remove_file(path.with_extension("json.tmp")).await;
            }
        }
    }
}

pub(super) struct SidecarState {
    pub(super) sidecar: TransferSidecar,
    pub(super) bits: Vec<u8>,
    /// Completed chunks not yet reflected in a persisted checkpoint.
    pub(super) unsaved_chunks: usize,
    pub(super) unsaved_bytes: u64,
    /// Completed chunks included in the last persisted checkpoint. Doubles as
    /// a dedupe guard so a lagging snapshot cannot be re-persisted forever.
    pub(super) persisted_chunks: usize,
    /// When the last checkpoint snapshot was queued for the writer (or the
    /// run started): backs the cadence save interval. Advanced only on a
    /// successful queue hand-off, so dropped snapshots leave it due.
    pub(super) last_checkpoint: Instant,
}

/// Checkpoint snapshot handed to the background writer: the encoded sidecar
/// plus the completed-chunk count it reflects.
pub(super) type CheckpointSnapshot = (TransferSidecar, usize);

/// Serializes checkpoint persistence off the transfer lanes. Lanes hand it
/// snapshots without blocking, it saves them one at a time and records the
/// persisted chunk count in the shared state, so a slow store never stalls
/// the data pipeline. [`CheckpointWriter::finish`] drains everything queued
/// before returning.
pub(super) struct CheckpointWriter {
    tx: Option<mpsc::Sender<CheckpointSnapshot>>,
    task: tokio::task::JoinHandle<()>,
}

impl CheckpointWriter {
    pub(super) fn spawn(state: Arc<Mutex<SidecarState>>, store: Arc<SidecarStore>) -> Self {
        let (tx, rx) = mpsc::channel(CHECKPOINT_QUEUE_CAPACITY);
        let task = tokio::spawn(checkpoint_writer(state, store, rx));
        Self { tx: Some(tx), task }
    }

    pub(super) fn sender(&self) -> Option<mpsc::Sender<CheckpointSnapshot>> {
        self.tx.clone()
    }

    /// Queue a snapshot, waiting for capacity: on the terminal flush path a
    /// dropped snapshot would discard acknowledged progress.
    pub(super) async fn send_flush(&self, snapshot: CheckpointSnapshot) {
        if let Some(tx) = self.tx.as_ref() {
            let _ = tx.send(snapshot).await;
        }
    }

    /// Stop accepting snapshots and wait until every queued one is saved.
    pub(super) async fn finish(mut self) {
        self.tx = None;
        let _ = self.task.await;
    }
}

async fn checkpoint_writer(
    state: Arc<Mutex<SidecarState>>,
    store: Arc<SidecarStore>,
    mut rx: mpsc::Receiver<CheckpointSnapshot>,
) {
    while let Some((sidecar, completed)) = rx.recv().await {
        // Checkpoint failures are deliberately non-fatal: the next cadence
        // snapshot or the cancel/error flush will retry it.
        if store.save(&sidecar).await.is_ok() {
            let mut state = state.lock().await;
            state.persisted_chunks = state.persisted_chunks.max(completed);
        }
    }
}

/// Whether a checkpoint save is due: either the unsaved byte threshold is
/// reached, or — bounding the re-send window on slow links — anything is
/// still unpersisted after the minimum interval. Chunk counts alone must not
/// trigger saves (see [`CHECKPOINT_MIN_BYTES`]).
pub(super) fn checkpoint_due(state: &SidecarState) -> bool {
    state.unsaved_bytes >= CHECKPOINT_MIN_BYTES
        || state.last_checkpoint.elapsed() >= CHECKPOINT_MIN_INTERVAL
}
