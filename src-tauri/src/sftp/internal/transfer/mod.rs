//! Resumable, chunked, parallel SFTP transfers.
//!
//! Every transfer writes to `<destination>.tterm.part` first and keeps a
//! sidecar (`.tterm.part.json`) describing the source fingerprint and the
//! bitmap of chunks that have been persisted. A resumed transfer only
//! re-sends the chunks that are still missing. When every chunk is present
//! the part file is renamed onto the final path (falling back to
//! remove-then-rename when the server refuses to overwrite), so the final
//! path never exposes a half-written file.
//!
//! The engine is transport agnostic: callers hand it one `RawSftpSession`
//! per parallel lane. That keeps the logic unit-testable against an
//! in-process `russh_sftp::server` (see `test_server`).
//!
//! Two properties keep the transfer fast on high-latency links:
//!
//! * Each lane keeps up to [`PIPELINE_WINDOW`] chunk requests in flight
//!   instead of waiting for every write/read acknowledgement, the way the
//!   OpenSSH `sftp` client does.
//! * The checkpoint sidecar is persisted by a background writer once
//!   [`CHECKPOINT_MIN_BYTES`] of newly acknowledged bytes have piled up —
//!   or, bounding the re-send window on slow links, whenever anything is
//!   still unpersisted after [`CHECKPOINT_MIN_INTERVAL`] — plus once on
//!   cancel or failure. Chunk data is only ever marked in the bitmap after
//!   the server acknowledged it, so a lagging checkpoint merely re-sends a
//!   few chunks after a crash — it can never stitch wrong bytes.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::RawSftpSession;
use russh_sftp::extensions::LimitsExtension;
use russh_sftp::protocol::{FileAttributes, OpenFlags, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinSet;

#[cfg(test)]
mod test_server;
#[cfg(test)]
mod tests;

pub const SIDECAR_VERSION: u32 = 1;
pub const SIDECAR_SUFFIX: &str = ".tterm.part.json";
pub const PART_SUFFIX: &str = ".tterm.part";
pub const DEFAULT_CHUNK_SIZE: u64 = 4 * 1024 * 1024;
pub const DEFAULT_PARALLELISM: usize = 4;
const MIN_CHUNK_SIZE: u64 = 4 * 1024;
/// Fallback step for a single SFTP read/write when the server does not
/// advertise `limits@openssh.com` (mirrors russh-sftp's own default).
const DEFAULT_IO_STEP: u64 = 256 * 1024;
pub const MAX_PARALLELISM: usize = 16;
/// Chunk requests one lane keeps in flight. SFTP multiplexes requests by id,
/// so a lane does not have to wait for each acknowledgement before sending
/// the next one; real servers (OpenSSH) process them concurrently.
const PIPELINE_WINDOW: usize = 8;
/// Checkpoint cadence: the sidecar is persisted once [`CHECKPOINT_MIN_BYTES`]
/// of *newly completed* bytes have piled up — or, bounding the re-send window
/// on slow links, whenever anything is still unpersisted after
/// [`CHECKPOINT_MIN_INTERVAL`] — plus once on cancel/failure. Chunk counts
/// alone must never trigger a save: with wire-limited chunks a handful of
/// them can add up to only a few MiB, while each save costs the store's
/// round trips.
const CHECKPOINT_MIN_BYTES: u64 = 32 * 1024 * 1024;
const CHECKPOINT_MIN_INTERVAL: Duration = Duration::from_secs(3);
/// Snapshot queue for the background checkpoint writer. When full, new
/// snapshots are dropped: a lagging checkpoint only widens the re-send
/// window after a crash, it can never stitch wrong bytes.
const CHECKPOINT_QUEUE_CAPACITY: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone)]
pub struct TransferOptions {
    pub parallelism: usize,
    pub chunk_size: u64,
    pub progress_interval_bytes: u64,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            parallelism: DEFAULT_PARALLELISM,
            chunk_size: DEFAULT_CHUNK_SIZE,
            progress_interval_bytes: 2 * 1024 * 1024,
        }
    }
}

impl TransferOptions {
    pub fn normalized(mut self) -> Self {
        self.parallelism = self.parallelism.clamp(1, MAX_PARALLELISM);
        self.chunk_size = self.chunk_size.max(MIN_CHUNK_SIZE);
        self
    }
}

#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub transferred: u64,
    pub total: u64,
    pub resumed_from: u64,
    pub parallelism: usize,
    pub completed_chunks: u64,
    pub chunk_count: u64,
}

pub type ProgressSink = Arc<dyn Fn(TransferProgress) + Send + Sync>;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TransferOutcome {
    pub resumed_from: u64,
    pub transferred: u64,
    pub total: u64,
    pub parallelism: usize,
}

#[derive(Debug)]
pub enum TransferError {
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for TransferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransferError::Cancelled => write!(f, "cancelled by user"),
            TransferError::Failed(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for TransferError {}

impl TransferError {
    pub fn is_cancelled(&self) -> bool {
        matches!(self, TransferError::Cancelled)
    }

    pub fn message(&self) -> String {
        self.to_string()
    }
}

fn failed(message: impl Into<String>) -> TransferError {
    TransferError::Failed(message.into())
}

/// One or more SFTP channels opened over the same SSH connection, plus the
/// server limits negotiated on that connection.
pub struct RemoteChannels {
    pub sessions: Vec<Arc<RawSftpSession>>,
    pub limits: Option<LimitsExtension>,
}

impl RemoteChannels {
    pub fn new(sessions: Vec<Arc<RawSftpSession>>, limits: Option<LimitsExtension>) -> Self {
        Self { sessions, limits }
    }

    /// A cheap, independent copy that shares the same channels. Used to run a
    /// batch of file transfers over one set of opened channels.
    pub fn instance(&self) -> Self {
        Self {
            sessions: self.sessions.clone(),
            limits: self.limits.as_ref().map(|limits| LimitsExtension {
                max_packet_len: limits.max_packet_len,
                max_read_len: limits.max_read_len,
                max_write_len: limits.max_write_len,
                max_open_handles: limits.max_open_handles,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSidecar {
    pub version: u32,
    pub direction: TransferDirection,
    pub final_path: String,
    pub part_path: String,
    pub chunk_size: u64,
    pub total_size: u64,
    pub source_size: u64,
    pub source_mtime: Option<i64>,
    /// Base64 encoding of a little-endian bitmap, one bit per chunk.
    pub completed: String,
    pub updated_at_ms: i64,
}

impl TransferSidecar {
    #[allow(clippy::too_many_arguments)]
    fn matches_source(
        &self,
        direction: TransferDirection,
        final_path: &str,
        part_path: &str,
        chunk_size: u64,
        source_size: u64,
        source_mtime: Option<i64>,
    ) -> bool {
        self.version == SIDECAR_VERSION
            && self.direction == direction
            && self.final_path == final_path
            && self.part_path == part_path
            && self.chunk_size == chunk_size
            && self.total_size == source_size
            && self.source_size == source_size
            && self.source_mtime == source_mtime
    }
}

pub fn part_path(final_path: &str) -> String {
    format!("{final_path}{PART_SUFFIX}")
}

/// Sidecar path for a transfer, derived from the final destination path:
/// `<final>.tterm.part.json` alongside the `<final>.tterm.part` file.
pub fn sidecar_path(final_path: &str) -> String {
    format!("{final_path}{SIDECAR_SUFFIX}")
}

pub fn chunk_count_for(total_size: u64, chunk_size: u64) -> usize {
    if total_size == 0 {
        return 0;
    }
    total_size.div_ceil(chunk_size) as usize
}

pub fn chunk_len_for(index: usize, chunk_size: u64, total_size: u64) -> u64 {
    let start = index as u64 * chunk_size;
    (total_size - start).min(chunk_size)
}

fn bitmap_len(chunk_count: usize) -> usize {
    chunk_count.div_ceil(8)
}

fn bitmap_get(bits: &[u8], index: usize) -> bool {
    bits.get(index / 8)
        .map(|byte| byte & (1 << (index % 8)) != 0)
        .unwrap_or(false)
}

fn bitmap_set(bits: &mut [u8], index: usize) {
    if let Some(byte) = bits.get_mut(index / 8) {
        *byte |= 1 << (index % 8);
    }
}

fn bitmap_count(bits: &[u8], chunk_count: usize) -> usize {
    (0..chunk_count)
        .filter(|index| bitmap_get(bits, *index))
        .count()
}

fn completed_bytes(bits: &[u8], chunk_size: u64, total_size: u64, chunk_count: usize) -> u64 {
    (0..chunk_count)
        .filter(|index| bitmap_get(bits, *index))
        .map(|index| chunk_len_for(index, chunk_size, total_size))
        .sum()
}

fn encode_bitmap(bits: &[u8]) -> String {
    BASE64.encode(bits)
}

fn decode_bitmap(encoded: &str, chunk_count: usize) -> Result<Vec<u8>, TransferError> {
    let decoded = BASE64
        .decode(encoded)
        .map_err(|err| failed(format!("Corrupt transfer checkpoint bitmap: {err}")))?;
    let mut bits = vec![0u8; bitmap_len(chunk_count)];
    for (index, byte) in decoded.iter().enumerate() {
        if index < bits.len() {
            bits[index] = *byte;
        }
    }
    Ok(bits)
}

fn step_from_limits(limits: Option<&LimitsExtension>) -> u64 {
    let mut step = 32 * 1024u64;
    if let Some(limits) = limits {
        if limits.max_write_len > 0 {
            step = step.min(limits.max_write_len);
        }
        if limits.max_read_len > 0 {
            step = step.min(limits.max_read_len);
        }
        if limits.max_packet_len > 0 {
            step = step.min(limits.max_packet_len.saturating_sub(1024));
        }
    }
    step.max(1)
}

/// Largest single write request a lane will issue for upload chunks.
fn io_write_step(limits: Option<&LimitsExtension>) -> u64 {
    limits
        .and_then(|limits| (limits.max_write_len > 0).then_some(limits.max_write_len))
        .unwrap_or(DEFAULT_IO_STEP)
}

/// Largest single read request a lane will issue for download chunks.
fn io_read_step(limits: Option<&LimitsExtension>) -> u64 {
    limits
        .and_then(|limits| (limits.max_read_len > 0).then_some(limits.max_read_len))
        .unwrap_or(DEFAULT_IO_STEP)
}

/// Chunk size after small-file shrinking.
///
/// A lane pipelines *chunks*, and each chunk sends its steps serially. When
/// the server does not advertise `limits@openssh.com` the logical chunk
/// (4 MiB) is far larger than one write step (256 KiB), so a file smaller
/// than a handful of chunks would run almost serially. Shrink the chunk to
/// `max(one write step, total / window)` in that case so the sliding window
/// becomes request-level pipelining. Never grows the chunk, never shrinks
/// below one write step, and leaves large transfers and empty transfers
/// alone: the chunk stays the logical 4 MiB resume unit even when the wire
/// step is smaller (the chunk is deliberately never clamped by the server's
/// read/write limits — `io_write_step`/`io_read_step` bound each request).
fn effective_chunk_size(negotiated: u64, step: u64, total_size: u64, window: usize) -> u64 {
    if total_size == 0 {
        return negotiated;
    }
    let target = (total_size / window.max(1) as u64).max(1);
    negotiated.min(step.max(target)).max(MIN_CHUNK_SIZE)
}

enum SidecarStore {
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
    async fn load(&self) -> Option<TransferSidecar> {
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

    async fn save(&self, sidecar: &TransferSidecar) -> Result<(), TransferError> {
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

    async fn remove(&self) {
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

async fn remote_read_all(
    session: &RawSftpSession,
    path: &str,
    step: u64,
) -> Result<Option<Vec<u8>>, TransferError> {
    let attrs = match session.stat(path).await {
        Ok(attrs) => attrs,
        Err(SftpError::Status(status)) if status.status_code == StatusCode::NoSuchFile => {
            return Ok(None)
        }
        Err(err) => return Err(failed(format!("Failed to stat '{path}': {err}"))),
    };

    let size = attrs.attrs.size.unwrap_or(0);
    if size == 0 {
        return Ok(Some(Vec::new()));
    }

    let handle = session
        .open(path, OpenFlags::READ, FileAttributes::empty())
        .await
        .map_err(|err| failed(format!("Failed to open '{path}' for reading: {err}")))?
        .handle;

    let mut buffer = Vec::with_capacity(size as usize);
    let step = step.clamp(1, u32::MAX as u64);

    while (buffer.len() as u64) < size {
        let remaining = size - buffer.len() as u64;
        let request = remaining.min(step) as u32;
        match session
            .read(handle.as_str(), buffer.len() as u64, request)
            .await
        {
            Ok(data) => {
                if data.data.is_empty() {
                    break;
                }
                buffer.extend_from_slice(&data.data);
            }
            Err(err) => {
                let _ = session.close(handle.as_str()).await;
                return Err(failed(format!("Failed to read '{path}': {err}")));
            }
        }
    }

    let _ = session.close(handle.as_str()).await;
    Ok(Some(buffer))
}

async fn remote_write_all(
    session: &RawSftpSession,
    path: &str,
    data: &[u8],
    step: u64,
) -> Result<(), TransferError> {
    let handle = session
        .open(
            path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            FileAttributes::empty(),
        )
        .await
        .map_err(|err| failed(format!("Failed to create '{path}': {err}")))?
        .handle;

    let step = step.clamp(1, u32::MAX as u64) as usize;
    let mut offset = 0usize;
    while offset < data.len() {
        let end = (offset + step).min(data.len());
        if let Err(err) = session
            .write(handle.as_str(), offset as u64, data[offset..end].to_vec())
            .await
        {
            let _ = session.close(handle.as_str()).await;
            return Err(failed(format!("Failed to write '{path}': {err}")));
        }
        offset = end;
    }

    let _ = session.close(handle.as_str()).await;
    Ok(())
}

async fn ensure_remote_dir_all(
    session: &RawSftpSession,
    remote_dir: &str,
) -> Result<(), TransferError> {
    let normalized = crate::sftp::internal::paths::normalize_remote_path(remote_dir);
    if normalized.is_empty() || normalized == "/" {
        return Ok(());
    }

    let mut current = if normalized.starts_with('/') {
        "/".to_string()
    } else {
        String::new()
    };

    for segment in normalized.split('/').filter(|segment| !segment.is_empty()) {
        current = crate::sftp::internal::paths::join_remote_path(&current, segment);
        // Stat before mkdir: for an already-existing tree this costs one
        // round trip per segment instead of a failing mkdir plus a
        // confirming stat.
        if session.stat(&current).await.is_ok() {
            continue;
        }
        if session
            .mkdir(&current, FileAttributes::empty())
            .await
            .is_err()
            && session.stat(&current).await.is_err()
        {
            return Err(failed(format!(
                "Failed to create remote directory '{current}'"
            )));
        }
    }

    Ok(())
}

async fn local_fingerprint(path: &Path) -> Result<(u64, Option<i64>), TransferError> {
    let metadata = tokio::fs::metadata(path).await.map_err(|err| {
        failed(format!(
            "Failed to read local file metadata '{}': {err}",
            path.display()
        ))
    })?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64);
    Ok((metadata.len(), mtime))
}

async fn remote_fingerprint(
    session: &RawSftpSession,
    path: &str,
) -> Result<Option<(u64, Option<i64>)>, TransferError> {
    match session.stat(path).await {
        Ok(attrs) => Ok(Some((
            attrs.attrs.size.unwrap_or(0),
            attrs.attrs.mtime.map(|mtime| mtime as i64),
        ))),
        Err(SftpError::Status(status)) if status.status_code == StatusCode::NoSuchFile => Ok(None),
        Err(err) => Err(failed(format!("Failed to stat remote '{path}': {err}"))),
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

struct SidecarState {
    sidecar: TransferSidecar,
    bits: Vec<u8>,
    /// Completed chunks not yet reflected in a persisted checkpoint.
    unsaved_chunks: usize,
    unsaved_bytes: u64,
    /// Completed chunks included in the last persisted checkpoint. Doubles as
    /// a dedupe guard so a lagging snapshot cannot be re-persisted forever.
    persisted_chunks: usize,
    /// When the last checkpoint snapshot was queued for the writer (or the
    /// run started): backs the cadence save interval. Advanced only on a
    /// successful queue hand-off, so dropped snapshots leave it due.
    last_checkpoint: Instant,
}

struct TransferPaths {
    final_path: String,
    part_path: String,
    sidecar_store: Arc<SidecarStore>,
    /// Local source path (upload) or local destination path (download).
    local_path: PathBuf,
    /// Remote destination path (upload) or remote source path (download).
    remote_path: String,
}

fn build_paths(
    direction: TransferDirection,
    local_path: &Path,
    remote_path: &str,
    limits: Option<&LimitsExtension>,
    control: Arc<RawSftpSession>,
) -> TransferPaths {
    match direction {
        TransferDirection::Upload => {
            let part = part_path(remote_path);
            let store = Arc::new(SidecarStore::Remote {
                session: control,
                path: sidecar_path(remote_path),
                step: step_from_limits(limits),
            });
            TransferPaths {
                final_path: remote_path.to_string(),
                part_path: part,
                sidecar_store: store,
                local_path: local_path.to_path_buf(),
                remote_path: remote_path.to_string(),
            }
        }
        TransferDirection::Download => {
            let part = PathBuf::from(part_path(&local_path.to_string_lossy()));
            let store = Arc::new(SidecarStore::Local {
                path: PathBuf::from(format!("{}{SIDECAR_SUFFIX}", local_path.to_string_lossy())),
            });
            TransferPaths {
                final_path: local_path.to_string_lossy().into_owned(),
                part_path: part.to_string_lossy().into_owned(),
                sidecar_store: store,
                local_path: local_path.to_path_buf(),
                remote_path: remote_path.to_string(),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_transfer(
    channels: RemoteChannels,
    direction: TransferDirection,
    local_path: PathBuf,
    remote_path: String,
    options: TransferOptions,
    mut cancel: watch::Receiver<bool>,
    progress: ProgressSink,
) -> Result<TransferOutcome, TransferError> {
    let options = options.normalized();

    if channels.sessions.is_empty() {
        return Err(failed("No SFTP channel available for transfer"));
    }

    let control = channels.sessions[0].clone();
    let paths = build_paths(
        direction,
        &local_path,
        &remote_path,
        channels.limits.as_ref(),
        control.clone(),
    );

    let (source_size, source_mtime) = match direction {
        TransferDirection::Upload => local_fingerprint(&paths.local_path).await?,
        TransferDirection::Download => remote_fingerprint(control.as_ref(), &paths.remote_path)
            .await?
            .ok_or_else(|| failed(format!("Remote file not found: {}", paths.remote_path)))?,
    };

    let total_size = source_size;
    // The chunk is the logical checkpoint/resume unit and is deliberately
    // not clamped by the server's wire limits: upload_chunk/download_chunk
    // split every chunk into step-sized requests (`io_write_step` /
    // `io_read_step`), so clamping here would only explode the chunk count
    // and the checkpoint cadence (e.g. OpenSSH's ~256 KiB write limit
    // turning one 4 MiB chunk into sixteen).
    let wire_step = match direction {
        TransferDirection::Upload => io_write_step(channels.limits.as_ref()),
        TransferDirection::Download => io_read_step(channels.limits.as_ref()),
    };
    let chunk_size =
        effective_chunk_size(options.chunk_size, wire_step, total_size, PIPELINE_WINDOW);
    let chunk_count = chunk_count_for(total_size, chunk_size);

    // Make sure the destination directory exists before touching the part file.
    match direction {
        TransferDirection::Upload => {
            if let Some(parent) =
                crate::sftp::internal::paths::parent_remote_path(&paths.final_path)
            {
                if !parent.is_empty() {
                    ensure_remote_dir_all(control.as_ref(), &parent).await?;
                }
            }
        }
        TransferDirection::Download => {
            if let Some(parent) = paths.local_path.parent() {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent).await.map_err(|err| {
                        failed(format!(
                            "Failed to create local directory '{}': {err}",
                            parent.display()
                        ))
                    })?;
                }
            }
        }
    }

    // Checkpointing only pays off when there is more than one chunk to
    // resume; for single-chunk (and empty) transfers it is pure per-file
    // overhead, so they skip the sidecar entirely and just retransmit on
    // retry.
    let checkpointing = chunk_count > 1;
    let mut bits = vec![0u8; bitmap_len(chunk_count)];
    let mut resumed_from = 0u64;

    if checkpointing {
        match paths.sidecar_store.load().await {
            Some(sidecar)
                if sidecar.matches_source(
                    direction,
                    &paths.final_path,
                    &paths.part_path,
                    chunk_size,
                    source_size,
                    source_mtime,
                ) =>
            {
                bits = decode_bitmap(&sidecar.completed, chunk_count)?;
                resumed_from = completed_bytes(&bits, chunk_size, total_size, chunk_count);
            }
            Some(_) | None => {
                // Baseline mismatch or a brand new transfer: never stitch
                // unrelated bytes together, always start from a clean part file.
                discard_partial(direction, control.as_ref(), &paths).await;
            }
        }
    } else {
        // Even without checkpointing, a stale part from an aborted older
        // attempt must not survive: writes cover [0, total) but a longer
        // stale tail would remain after the final rename.
        discard_partial(direction, control.as_ref(), &paths).await;
    }

    let mut missing: Vec<usize> = (0..chunk_count)
        .filter(|index| !bitmap_get(&bits, *index))
        .collect();

    // A resumed checkpoint is only trustworthy if the part file still
    // contains every byte the bitmap claims. If the part file went missing
    // or was truncated out-of-band while the checkpoint survived, writing
    // only the missing chunks would stitch zero-filled holes into the file
    // that is about to be renamed onto the final path.
    if checkpointing
        && bitmap_count(&bits, chunk_count) > 0
        && !part_covers_completed(
            direction,
            control.as_ref(),
            &paths,
            &bits,
            chunk_size,
            total_size,
            chunk_count,
        )
        .await
    {
        discard_partial(direction, control.as_ref(), &paths).await;
        bits = vec![0u8; bitmap_len(chunk_count)];
        resumed_from = 0;
        missing = (0..chunk_count).collect();
    }

    let initial_completed_chunks = bitmap_count(&bits, chunk_count) as u64;
    let sidecar = TransferSidecar {
        version: SIDECAR_VERSION,
        direction,
        final_path: paths.final_path.clone(),
        part_path: paths.part_path.clone(),
        chunk_size,
        total_size,
        source_size,
        source_mtime,
        completed: encode_bitmap(&bits),
        updated_at_ms: now_unix_ms(),
    };

    if missing.is_empty() {
        // Checkpoint already complete: make sure the part file exists (it may
        // be a zero-byte transfer, or a stale part was discarded above) and
        // finalize without any chunk I/O. No checkpoint write is needed —
        // finalize removes the sidecar right after the rename.
        ensure_part_exists(direction, control.as_ref(), &paths).await?;
        finalize(direction, control.as_ref(), &paths).await?;
        return Ok(TransferOutcome {
            resumed_from,
            transferred: resumed_from,
            total: total_size,
            parallelism: options.parallelism,
        });
    }

    emit_progress(
        &progress,
        total_size,
        chunk_count,
        resumed_from,
        options.parallelism,
        initial_completed_chunks,
        resumed_from,
    );

    let run = Arc::new(TransferRun {
        direction,
        chunks: missing,
        chunk_size,
        total_size,
        chunk_count,
        resumed_from,
        initial_completed_chunks,
        checkpointing,
        sidecar_state: Arc::new(Mutex::new(SidecarState {
            sidecar,
            bits,
            unsaved_chunks: 0,
            unsaved_bytes: 0,
            persisted_chunks: initial_completed_chunks as usize,
            last_checkpoint: Instant::now(),
        })),
        sidecar_store: paths.sidecar_store.clone(),
    });

    // Checkpoint persistence runs on a dedicated writer so the lanes never
    // stall on the store's round trips; `finish` below drains everything
    // queued before the transfer reports its result.
    let checkpoint = run
        .checkpointing
        .then(|| CheckpointWriter::spawn(run.sidecar_state.clone(), run.sidecar_store.clone()));

    if let Err(err) = execute_parallel(
        &channels,
        &paths,
        &run,
        &options,
        &mut cancel,
        &progress,
        checkpoint.as_ref(),
    )
    .await
    {
        // Best-effort flush so every acknowledged chunk survives the retry;
        // check(point) failures are not allowed to mask the transfer error.
        if let Some(writer) = checkpoint.as_ref() {
            flush_checkpoint(&run, writer).await;
        }
        if let Some(writer) = checkpoint {
            writer.finish().await;
        }
        return Err(err);
    }

    if let Some(writer) = checkpoint {
        // Drain any straggling snapshot before finalizing so a late save can
        // not resurrect the sidecar after the rename removed it.
        writer.finish().await;
    }

    // Everything should be present now; guard against a lost update.
    let (completed_chunks, transferred) = {
        let state = run.sidecar_state.lock().await;
        let present = bitmap_count(&state.bits, chunk_count);
        if present != chunk_count {
            return Err(failed(format!(
                "Transfer incomplete: {present}/{chunk_count} chunks present"
            )));
        }
        let transferred: u64 = (0..chunk_count)
            .filter(|index| bitmap_get(&state.bits, *index))
            .map(|index| chunk_len_for(index, chunk_size, total_size))
            .sum();
        (present as u64, transferred)
    };

    emit_progress(
        &progress,
        total_size,
        chunk_count,
        resumed_from,
        options.parallelism,
        completed_chunks,
        transferred,
    );

    finalize(direction, control.as_ref(), &paths).await?;

    Ok(TransferOutcome {
        resumed_from,
        transferred,
        total: total_size,
        parallelism: options.parallelism,
    })
}

#[allow(clippy::too_many_arguments)]
fn emit_progress(
    progress: &ProgressSink,
    total: u64,
    chunk_count: usize,
    resumed_from: u64,
    parallelism: usize,
    completed_chunks: u64,
    transferred: u64,
) {
    progress(TransferProgress {
        transferred,
        total,
        resumed_from,
        parallelism,
        completed_chunks,
        chunk_count: chunk_count as u64,
    });
}

struct TransferRun {
    direction: TransferDirection,
    chunks: Vec<usize>,
    chunk_size: u64,
    total_size: u64,
    chunk_count: usize,
    resumed_from: u64,
    initial_completed_chunks: u64,
    checkpointing: bool,
    sidecar_state: Arc<Mutex<SidecarState>>,
    sidecar_store: Arc<SidecarStore>,
}

/// Checkpoint snapshot handed to the background writer: the encoded sidecar
/// plus the completed-chunk count it reflects.
type CheckpointSnapshot = (TransferSidecar, usize);

/// Serializes checkpoint persistence off the transfer lanes. Lanes hand it
/// snapshots without blocking, it saves them one at a time and records the
/// persisted chunk count in the shared state, so a slow store never stalls
/// the data pipeline. [`CheckpointWriter::finish`] drains everything queued
/// before returning.
struct CheckpointWriter {
    tx: Option<mpsc::Sender<CheckpointSnapshot>>,
    task: tokio::task::JoinHandle<()>,
}

impl CheckpointWriter {
    fn spawn(state: Arc<Mutex<SidecarState>>, store: Arc<SidecarStore>) -> Self {
        let (tx, rx) = mpsc::channel(CHECKPOINT_QUEUE_CAPACITY);
        let task = tokio::spawn(checkpoint_writer(state, store, rx));
        Self { tx: Some(tx), task }
    }

    fn sender(&self) -> Option<mpsc::Sender<CheckpointSnapshot>> {
        self.tx.clone()
    }

    /// Queue a snapshot, waiting for capacity: on the terminal flush path a
    /// dropped snapshot would discard acknowledged progress.
    async fn send_flush(&self, snapshot: CheckpointSnapshot) {
        if let Some(tx) = self.tx.as_ref() {
            let _ = tx.send(snapshot).await;
        }
    }

    /// Stop accepting snapshots and wait until every queued one is saved.
    async fn finish(mut self) {
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

/// Highest offset the part file must cover for the completed bitmap to be
/// believable: the end of the last completed chunk (the final chunk ends at
/// `total_size`).
fn required_part_size(bits: &[u8], chunk_size: u64, total_size: u64, chunk_count: usize) -> u64 {
    (0..chunk_count)
        .filter(|index| bitmap_get(bits, *index))
        .map(|index| ((index as u64 + 1) * chunk_size).min(total_size))
        .max()
        .unwrap_or(0)
}

/// Whether the part file matches what the completed bitmap claims. A smaller
/// part file means the checkpoint and the part file are out of sync (e.g. the
/// part file was deleted or truncated out-of-band) and the transfer must
/// restart from scratch. A *larger* part file is just as unusable: parts are
/// opened without truncation, so a stale longer tail from an older, bigger
/// attempt would survive the final rename as trailing garbage.
async fn part_covers_completed(
    direction: TransferDirection,
    control: &RawSftpSession,
    paths: &TransferPaths,
    bits: &[u8],
    chunk_size: u64,
    total_size: u64,
    chunk_count: usize,
) -> bool {
    let required = required_part_size(bits, chunk_size, total_size, chunk_count);
    if required == 0 {
        return true;
    }
    let part_size = match direction {
        TransferDirection::Upload => match control.stat(&paths.part_path).await {
            Ok(attrs) => attrs.attrs.size.unwrap_or(0),
            Err(_) => return false,
        },
        TransferDirection::Download => match tokio::fs::metadata(&paths.part_path).await {
            Ok(metadata) => metadata.len(),
            Err(_) => return false,
        },
    };
    part_size >= required && part_size <= total_size
}

async fn ensure_part_exists(
    direction: TransferDirection,
    control: &RawSftpSession,
    paths: &TransferPaths,
) -> Result<(), TransferError> {
    match direction {
        TransferDirection::Upload => {
            let handle = control
                .open(
                    &paths.part_path,
                    OpenFlags::CREATE | OpenFlags::WRITE,
                    FileAttributes::empty(),
                )
                .await
                .map_err(|err| {
                    failed(format!(
                        "Failed to create remote part '{}': {err}",
                        paths.part_path
                    ))
                })?
                .handle;
            let _ = control.close(handle.as_str()).await;
        }
        TransferDirection::Download => {
            tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(PathBuf::from(&paths.part_path))
                .await
                .map_err(|err| {
                    failed(format!(
                        "Failed to create local part '{}': {err}",
                        paths.part_path
                    ))
                })?;
        }
    }
    Ok(())
}

/// Remove the part file and the sidecar so the next attempt starts clean.
///
/// The remove must not silently fail: parts are opened without truncation,
/// so a stale part from an older, larger attempt that survives a failed
/// remove would carry its tail through the final rename as trailing
/// garbage. When the remove fails the part is truncated in place instead —
/// and if even that fails the part cannot be modified at all, in which case
/// the transfer is about to fail opening it for writing anyway.
async fn discard_partial(
    direction: TransferDirection,
    control: &RawSftpSession,
    paths: &TransferPaths,
) {
    match direction {
        TransferDirection::Upload => {
            let gone = match control.remove(&paths.part_path).await {
                Ok(_) => true,
                Err(SftpError::Status(status)) => status.status_code == StatusCode::NoSuchFile,
                Err(_) => false,
            };
            if !gone {
                if let Ok(opened) = control
                    .open(
                        &paths.part_path,
                        OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE,
                        FileAttributes::empty(),
                    )
                    .await
                {
                    let _ = control.close(opened.handle.as_str()).await;
                }
            }
        }
        TransferDirection::Download => {
            let gone = match tokio::fs::remove_file(&paths.local_path_part()).await {
                Ok(()) => true,
                Err(err) => err.kind() == std::io::ErrorKind::NotFound,
            };
            if !gone {
                let _ = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&paths.local_path_part())
                    .await;
            }
        }
    }
    paths.sidecar_store.remove().await;
}

impl TransferPaths {
    fn local_path_part(&self) -> PathBuf {
        PathBuf::from(&self.part_path)
    }
}

async fn execute_parallel(
    channels: &RemoteChannels,
    paths: &TransferPaths,
    run: &Arc<TransferRun>,
    options: &TransferOptions,
    cancel: &mut watch::Receiver<bool>,
    progress: &ProgressSink,
    checkpoint: Option<&CheckpointWriter>,
) -> Result<(), TransferError> {
    let mut lanes = options.parallelism.min(channels.sessions.len()).max(1);
    if let Some(limits) = channels.limits.as_ref() {
        if limits.max_open_handles > 0 {
            // Every lane holds one handle for the whole transfer; reserve one
            // more for the control session's checkpoint/metadata traffic.
            lanes = lanes
                .min((limits.max_open_handles as usize).saturating_sub(1))
                .max(1);
        }
    }

    let queue = Arc::new(Mutex::new(VecDeque::from(run.chunks.clone())));
    let shared = Arc::new(LaneShared {
        queue: queue.clone(),
        transferred: Arc::new(AtomicU64::new(run.resumed_from)),
        completed_chunks: Arc::new(AtomicU64::new(run.initial_completed_chunks)),
        progress: progress.clone(),
        cancel: cancel.clone(),
        chunk_size: run.chunk_size,
        total_size: run.total_size,
        chunk_count: run.chunk_count,
        resumed_from: run.resumed_from,
        parallelism: options.parallelism,
        progress_interval: options.progress_interval_bytes,
        sidecar_state: run.sidecar_state.clone(),
        local_source: paths.local_path.clone(),
        local_part: PathBuf::from(&paths.part_path),
        remote_part: paths.part_path.clone(),
        remote_source: paths.remote_path.clone(),
        direction: run.direction,
        read_step: io_read_step(channels.limits.as_ref()),
        write_step: io_write_step(channels.limits.as_ref()),
        checkpointing: run.checkpointing,
        checkpoint_tx: checkpoint.and_then(|writer| writer.sender()),
    });

    let mut join_set = tokio::task::JoinSet::new();

    for lane in 0..lanes {
        let session = channels.sessions[lane % channels.sessions.len()].clone();
        join_set.spawn(lane_worker(session, shared.clone()));
    }

    let mut first_error: Option<TransferError> = None;
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                if first_error.is_none() {
                    first_error = Some(err);
                }
                join_set.abort_all();
                break;
            }
            Err(join_err) => {
                if first_error.is_none() {
                    first_error = Some(failed(format!("Transfer worker failed: {join_err}")));
                }
                join_set.abort_all();
                break;
            }
        }
    }

    while join_set.join_next().await.is_some() {}

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// One local file handle per lane, shared by the lane's chunk tasks. The
/// mutex serializes the positioned reads/writes (a seekable handle cannot be
/// used concurrently without one) but is only ever held across a local
/// read/write, never across a network round trip, so the lane's pipeline
/// stays overlapped while the file is no longer reopened for every chunk.
type LocalLaneFile = Arc<Mutex<tokio::fs::File>>;

/// State shared by every lane of one transfer (each lane pairs it with its
/// own `RawSftpSession`).
struct LaneShared {
    queue: Arc<Mutex<VecDeque<usize>>>,
    transferred: Arc<AtomicU64>,
    completed_chunks: Arc<AtomicU64>,
    progress: ProgressSink,
    cancel: watch::Receiver<bool>,
    chunk_size: u64,
    total_size: u64,
    chunk_count: usize,
    resumed_from: u64,
    parallelism: usize,
    progress_interval: u64,
    sidecar_state: Arc<Mutex<SidecarState>>,
    local_source: PathBuf,
    local_part: PathBuf,
    remote_part: String,
    remote_source: String,
    direction: TransferDirection,
    read_step: u64,
    write_step: u64,
    checkpointing: bool,
    checkpoint_tx: Option<mpsc::Sender<CheckpointSnapshot>>,
}

async fn lane_worker(
    session: Arc<RawSftpSession>,
    shared: Arc<LaneShared>,
) -> Result<(), TransferError> {
    let handle = match shared.direction {
        TransferDirection::Upload => {
            open_upload_part(session.as_ref(), &shared.remote_part).await?
        }
        TransferDirection::Download => {
            open_download_source(session.as_ref(), &shared.remote_source).await?
        }
    };

    let local_file: LocalLaneFile = match shared.direction {
        TransferDirection::Upload => {
            let file = tokio::fs::File::open(&shared.local_source)
                .await
                .map_err(|err| {
                    failed(format!(
                        "Failed to open local source '{}': {err}",
                        shared.local_source.display()
                    ))
                })?;
            Arc::new(Mutex::new(file))
        }
        TransferDirection::Download => {
            let file = tokio::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(&shared.local_part)
                .await
                .map_err(|err| {
                    failed(format!(
                        "Failed to open local part '{}': {err}",
                        shared.local_part.display()
                    ))
                })?;
            Arc::new(Mutex::new(file))
        }
    };

    // Sliding window of chunk tasks. Keeping several requests in flight
    // hides the per-request round trip on high-latency links; each task
    // reports its chunk index so only fully acknowledged chunks are marked.
    let mut chunk_tasks: JoinSet<(usize, Result<(), TransferError>)> = JoinSet::new();
    let mut last_progress = shared.transferred.load(Ordering::Relaxed);

    let result: Result<(), TransferError> = loop {
        while chunk_tasks.len() < PIPELINE_WINDOW {
            if *shared.cancel.borrow() {
                break;
            }
            let index = {
                let mut queue = shared.queue.lock().await;
                queue.pop_front()
            };
            let Some(index) = index else { break };
            chunk_tasks.spawn(transfer_chunk(
                session.clone(),
                shared.clone(),
                handle.clone(),
                local_file.clone(),
                index,
            ));
        }

        // Checked before draining completions so a cancel triggered by the
        // progress callback stops marking chunks deterministically.
        if *shared.cancel.borrow() {
            break Err(TransferError::Cancelled);
        }

        let Some(joined) = chunk_tasks.join_next().await else {
            // Window empty and queue drained.
            break Ok(());
        };

        match joined {
            Ok((index, Ok(()))) => {
                record_chunk_completion(&shared, index, &mut last_progress).await;
            }
            Ok((_, Err(err))) => break Err(err),
            Err(join_err) => break Err(failed(format!("Transfer worker failed: {join_err}"))),
        }
    };

    chunk_tasks.abort_all();
    while chunk_tasks.join_next().await.is_some() {}
    let _ = session.close(handle.as_str()).await;

    result
}

/// Move one acknowledged chunk into the bitmap (throttled checkpoint) and
/// emit progress. The chunk data is already durable at this point, so a
/// failed checkpoint save is deliberately non-fatal: the next cadence save
/// or the cancel/error flush will retry it.
async fn record_chunk_completion(shared: &LaneShared, index: usize, last_progress: &mut u64) {
    let len = chunk_len_for(index, shared.chunk_size, shared.total_size);
    let snapshot = {
        let mut state = shared.sidecar_state.lock().await;
        if bitmap_get(&state.bits, index) {
            None
        } else {
            bitmap_set(&mut state.bits, index);
            shared.completed_chunks.fetch_add(1, Ordering::SeqCst);
            state.unsaved_chunks += 1;
            state.unsaved_bytes += len;
            let completed = bitmap_count(&state.bits, shared.chunk_count);
            if shared.checkpointing && completed > state.persisted_chunks && checkpoint_due(&state)
            {
                state.unsaved_chunks = 0;
                state.unsaved_bytes = 0;
                state.sidecar.completed = encode_bitmap(&state.bits);
                state.sidecar.updated_at_ms = now_unix_ms();
                Some((state.sidecar.clone(), completed))
            } else {
                None
            }
        }
    };

    if let Some(snapshot) = snapshot {
        // Handed to the background checkpoint writer without blocking: when
        // its queue is full the snapshot is dropped, which only widens the
        // re-send window after a crash — never the byte correctness.
        if let Some(tx) = shared.checkpoint_tx.as_ref() {
            if tx.try_send(snapshot).is_ok() {
                // The cadence clock advances only once the snapshot is
                // actually queued: a dropped one must leave the interval
                // backstop due, so the next completion retries the hand-off
                // instead of waiting out a fresh interval.
                let mut state = shared.sidecar_state.lock().await;
                state.last_checkpoint = Instant::now();
            }
        }
    }

    let total = shared.transferred.fetch_add(len, Ordering::SeqCst) + len;
    if total.saturating_sub(*last_progress) >= shared.progress_interval
        || total >= shared.total_size
    {
        *last_progress = total;
        (shared.progress)(TransferProgress {
            transferred: total,
            total: shared.total_size,
            resumed_from: shared.resumed_from,
            parallelism: shared.parallelism,
            completed_chunks: shared.completed_chunks.load(Ordering::SeqCst),
            chunk_count: shared.chunk_count as u64,
        });
    }
}

/// Whether a checkpoint save is due: either the unsaved byte threshold is
/// reached, or — bounding the re-send window on slow links — anything is
/// still unpersisted after the minimum interval. Chunk counts alone must not
/// trigger saves (see [`CHECKPOINT_MIN_BYTES`]).
fn checkpoint_due(state: &SidecarState) -> bool {
    state.unsaved_bytes >= CHECKPOINT_MIN_BYTES
        || state.last_checkpoint.elapsed() >= CHECKPOINT_MIN_INTERVAL
}

/// Persist the current bitmap unconditionally (cancel/failure path) through
/// the background writer. Skipped when the on-disk checkpoint already
/// reflects every marked chunk.
async fn flush_checkpoint(run: &TransferRun, writer: &CheckpointWriter) {
    let snapshot = {
        let mut state = run.sidecar_state.lock().await;
        let completed = bitmap_count(&state.bits, run.chunk_count);
        if completed <= state.persisted_chunks {
            return;
        }
        state.sidecar.completed = encode_bitmap(&state.bits);
        state.sidecar.updated_at_ms = now_unix_ms();
        (state.sidecar.clone(), completed)
    };
    writer.send_flush(snapshot).await;
}

/// Transfer a single chunk: upload reads the local source and writes remote
/// steps; download reads remote steps and writes the local part. Runs as its
/// own task so several chunks can be in flight per lane.
async fn transfer_chunk(
    session: Arc<RawSftpSession>,
    shared: Arc<LaneShared>,
    handle: String,
    local_file: LocalLaneFile,
    index: usize,
) -> (usize, Result<(), TransferError>) {
    let offset = index as u64 * shared.chunk_size;
    let len = chunk_len_for(index, shared.chunk_size, shared.total_size);
    let result = match shared.direction {
        TransferDirection::Upload => {
            upload_chunk(&shared, session.as_ref(), &handle, &local_file, offset, len).await
        }
        TransferDirection::Download => {
            download_chunk(&shared, session.as_ref(), &handle, &local_file, offset, len).await
        }
    };
    (index, result)
}

async fn upload_chunk(
    shared: &LaneShared,
    session: &RawSftpSession,
    handle: &str,
    local_file: &LocalLaneFile,
    offset: u64,
    len: u64,
) -> Result<(), TransferError> {
    let mut buffer = vec![0u8; shared.write_step as usize];
    let mut pos = 0u64;
    while pos < len {
        if *shared.cancel.borrow() {
            return Err(TransferError::Cancelled);
        }
        let want = (len - pos).min(shared.write_step) as usize;
        {
            // Lock only for the positioned local read; the network write
            // below happens without the lock so sibling chunk tasks keep
            // writing while this request is in flight.
            let mut reader = local_file.lock().await;
            reader
                .seek(std::io::SeekFrom::Start(offset + pos))
                .await
                .map_err(|err| failed(format!("Failed to seek local source: {err}")))?;
            reader
                .read_exact(&mut buffer[..want])
                .await
                .map_err(|err| {
                    failed(format!(
                        "Failed to read local source at offset {}: {err}",
                        offset + pos
                    ))
                })?;
        }
        session
            .write(handle, offset + pos, buffer[..want].to_vec())
            .await
            .map_err(|err| failed(format!("Failed to write remote part: {err}")))?;
        pos += want as u64;
    }
    Ok(())
}

async fn download_chunk(
    shared: &LaneShared,
    session: &RawSftpSession,
    handle: &str,
    local_file: &LocalLaneFile,
    offset: u64,
    len: u64,
) -> Result<(), TransferError> {
    let step = shared.read_step.clamp(1, u32::MAX as u64);
    let mut pos = 0u64;
    while pos < len {
        if *shared.cancel.borrow() {
            return Err(TransferError::Cancelled);
        }
        let request = (len - pos).min(step) as u32;
        let data = session
            .read(handle, offset + pos, request)
            .await
            .map_err(|err| failed(format!("Failed to read remote source: {err}")))?;
        if data.data.is_empty() {
            return Err(failed(format!(
                "Short read from remote source at offset {offset}: got {pos} of {len} bytes"
            )));
        }
        {
            // Lock only for the positioned local write; the next network
            // read below happens without the lock.
            let mut writer = local_file.lock().await;
            writer
                .seek(std::io::SeekFrom::Start(offset + pos))
                .await
                .map_err(|err| failed(format!("Failed to seek local part: {err}")))?;
            writer
                .write_all(&data.data)
                .await
                .map_err(|err| failed(format!("Failed to write local part: {err}")))?;
        }
        pos += data.data.len() as u64;
    }
    Ok(())
}

async fn open_upload_part(session: &RawSftpSession, path: &str) -> Result<String, TransferError> {
    session
        .open(
            path,
            OpenFlags::CREATE | OpenFlags::WRITE,
            FileAttributes::empty(),
        )
        .await
        .map(|handle| handle.handle)
        .map_err(|err| failed(format!("Failed to open remote part '{path}': {err}")))
}

async fn open_download_source(
    session: &RawSftpSession,
    path: &str,
) -> Result<String, TransferError> {
    session
        .open(path, OpenFlags::READ, FileAttributes::empty())
        .await
        .map(|handle| handle.handle)
        .map_err(|err| failed(format!("Failed to open remote source '{path}': {err}")))
}

async fn finalize(
    direction: TransferDirection,
    control: &RawSftpSession,
    paths: &TransferPaths,
) -> Result<(), TransferError> {
    match direction {
        TransferDirection::Upload => {
            if let Err(err) = control.rename(&paths.part_path, &paths.final_path).await {
                let _ = control.remove(&paths.final_path).await;
                control
                    .rename(&paths.part_path, &paths.final_path)
                    .await
                    .map_err(|retry| {
                        failed(format!(
                            "Failed to finalize remote file (rename error: {err}; retry error: {retry})"
                        ))
                    })?;
            }
        }
        TransferDirection::Download => {
            let part = PathBuf::from(&paths.part_path);
            let final_path = PathBuf::from(&paths.final_path);
            if tokio::fs::rename(&part, &final_path).await.is_err() {
                let _ = tokio::fs::remove_file(&final_path).await;
                tokio::fs::rename(&part, &final_path)
                    .await
                    .map_err(|err| failed(format!("Failed to finalize local file: {err}")))?;
            }
        }
    }

    paths.sidecar_store.remove().await;
    Ok(())
}

/// Upload `local_path` to `remote_path` using `channels` for parallel chunk I/O.
pub async fn upload_file(
    channels: RemoteChannels,
    local_path: PathBuf,
    remote_path: String,
    options: TransferOptions,
    cancel: watch::Receiver<bool>,
    progress: ProgressSink,
) -> Result<TransferOutcome, TransferError> {
    run_transfer(
        channels,
        TransferDirection::Upload,
        local_path,
        remote_path,
        options,
        cancel,
        progress,
    )
    .await
}

/// Download `remote_path` to `local_path` using `channels` for parallel chunk I/O.
pub async fn download_file(
    channels: RemoteChannels,
    local_path: PathBuf,
    remote_path: String,
    options: TransferOptions,
    cancel: watch::Receiver<bool>,
    progress: ProgressSink,
) -> Result<TransferOutcome, TransferError> {
    run_transfer(
        channels,
        TransferDirection::Download,
        local_path,
        remote_path,
        options,
        cancel,
        progress,
    )
    .await
}
