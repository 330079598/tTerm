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

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::RawSftpSession;
use russh_sftp::extensions::LimitsExtension;
use russh_sftp::protocol::{FileAttributes, OpenFlags, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{watch, Mutex};

#[cfg(test)]
mod test_server;
#[cfg(test)]
mod tests;

pub const SIDECAR_VERSION: u32 = 1;
pub const SIDECAR_SUFFIX: &str = ".tterm.part.json";
pub const PART_SUFFIX: &str = ".tterm.part";
pub const DEFAULT_CHUNK_SIZE: u64 = 4 * 1024 * 1024;
const MIN_CHUNK_SIZE: u64 = 4 * 1024;
/// Fallback step for a single SFTP read/write when the server does not
/// advertise `limits@openssh.com` (mirrors russh-sftp's own default).
const DEFAULT_IO_STEP: u64 = 256 * 1024;
const MAX_PARALLELISM: usize = 16;

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
            parallelism: 4,
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

/// The chunk size we will actually use, clamped by `limits@openssh.com`.
pub fn negotiate_chunk_size(desired: u64, limits: Option<&LimitsExtension>) -> u64 {
    let mut chunk = desired.max(MIN_CHUNK_SIZE);
    if let Some(limits) = limits {
        if limits.max_read_len > 0 {
            chunk = chunk.min(limits.max_read_len);
        }
        if limits.max_write_len > 0 {
            chunk = chunk.min(limits.max_write_len);
        }
        if limits.max_packet_len > 0 {
            // Leave room for the packet header/handle overhead.
            chunk = chunk.min(
                limits
                    .max_packet_len
                    .saturating_sub(1024)
                    .max(MIN_CHUNK_SIZE),
            );
        }
    }
    chunk.max(MIN_CHUNK_SIZE)
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
                let data = remote_read_all(session.as_ref(), path, *step)
                    .await
                    .ok()??;
                serde_json::from_slice(&data).ok()
            }
            SidecarStore::Local { path } => {
                let data = tokio::fs::read(path).await.ok()?;
                serde_json::from_slice(&data).ok()
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
            } => remote_write_all(session.as_ref(), path, &data, *step).await,
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
            }
            SidecarStore::Local { path } => {
                let _ = tokio::fs::remove_file(path).await;
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
    let chunk_size = negotiate_chunk_size(options.chunk_size, channels.limits.as_ref());
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

    let existing = paths.sidecar_store.load().await;
    let mut bits = vec![0u8; bitmap_len(chunk_count)];
    let mut resumed_from = 0u64;

    match existing {
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

    let mut missing: Vec<usize> = (0..chunk_count)
        .filter(|index| !bitmap_get(&bits, *index))
        .collect();

    // A checkpoint that claims completion is only trustworthy if the part file
    // is actually large enough; otherwise we would rename a truncated file.
    if missing.is_empty()
        && total_size > 0
        && !part_is_complete(direction, control.as_ref(), &paths, total_size).await
    {
        discard_partial(direction, control.as_ref(), &paths).await;
        bits = vec![0u8; bitmap_len(chunk_count)];
        resumed_from = 0;
        missing = (0..chunk_count).collect();
    }

    let initial_completed_chunks = bitmap_count(&bits, chunk_count) as u64;
    let mut sidecar = TransferSidecar {
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
        // be a zero-byte transfer, or a stale part was discarded above),
        // persist the checkpoint and finalize without any chunk I/O.
        ensure_part_exists(direction, control.as_ref(), &paths).await?;
        paths.sidecar_store.save(&sidecar).await?;
        finalize(direction, control.as_ref(), &paths).await?;
        return Ok(TransferOutcome {
            resumed_from,
            transferred: resumed_from,
            total: total_size,
            parallelism: options.parallelism,
        });
    }

    sidecar.updated_at_ms = now_unix_ms();
    paths.sidecar_store.save(&sidecar).await?;
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
        sidecar_state: Arc::new(Mutex::new(SidecarState { sidecar, bits })),
        sidecar_store: paths.sidecar_store.clone(),
    });

    execute_parallel(&channels, &paths, &run, &options, &mut cancel, &progress).await?;

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
    sidecar_state: Arc<Mutex<SidecarState>>,
    sidecar_store: Arc<SidecarStore>,
}

async fn part_is_complete(
    direction: TransferDirection,
    control: &RawSftpSession,
    paths: &TransferPaths,
    total_size: u64,
) -> bool {
    match direction {
        TransferDirection::Upload => match control.stat(&paths.part_path).await {
            Ok(attrs) => attrs.attrs.size.unwrap_or(0) >= total_size,
            Err(_) => false,
        },
        TransferDirection::Download => match tokio::fs::metadata(&paths.part_path).await {
            Ok(metadata) => metadata.len() >= total_size,
            Err(_) => false,
        },
    }
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

async fn discard_partial(
    direction: TransferDirection,
    control: &RawSftpSession,
    paths: &TransferPaths,
) {
    match direction {
        TransferDirection::Upload => {
            let _ = control.remove(&paths.part_path).await;
        }
        TransferDirection::Download => {
            let _ = tokio::fs::remove_file(&paths.local_path_part()).await;
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
) -> Result<(), TransferError> {
    let lanes = options.parallelism.min(channels.sessions.len()).max(1);
    let queue = Arc::new(Mutex::new(VecDeque::from(run.chunks.clone())));
    let transferred = Arc::new(AtomicU64::new(run.resumed_from));
    let completed_chunks = Arc::new(AtomicU64::new(run.initial_completed_chunks));
    let total_size = run.total_size;
    let resumed_from = run.resumed_from;

    let mut join_set = tokio::task::JoinSet::new();

    for lane in 0..lanes {
        let session = channels.sessions[lane % channels.sessions.len()].clone();
        let ctx = LaneContext {
            session,
            queue: queue.clone(),
            transferred: transferred.clone(),
            completed_chunks: completed_chunks.clone(),
            progress: progress.clone(),
            cancel: cancel.clone(),
            chunk_size: run.chunk_size,
            total_size,
            chunk_count: run.chunk_count,
            resumed_from,
            parallelism: options.parallelism,
            progress_interval: options.progress_interval_bytes,
            sidecar_state: run.sidecar_state.clone(),
            sidecar_store: run.sidecar_store.clone(),
            local_source: paths.local_path.clone(),
            local_part: PathBuf::from(&paths.part_path),
            remote_part: paths.part_path.clone(),
            remote_source: paths.remote_path.clone(),
            direction: run.direction,
            read_limit: channels
                .limits
                .as_ref()
                .and_then(|limits| (limits.max_read_len > 0).then_some(limits.max_read_len)),
            write_limit: channels
                .limits
                .as_ref()
                .and_then(|limits| (limits.max_write_len > 0).then_some(limits.max_write_len)),
        };
        join_set.spawn(lane_worker(ctx));
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

struct LaneContext {
    session: Arc<RawSftpSession>,
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
    sidecar_store: Arc<SidecarStore>,
    local_source: PathBuf,
    local_part: PathBuf,
    remote_part: String,
    remote_source: String,
    direction: TransferDirection,
    read_limit: Option<u64>,
    write_limit: Option<u64>,
}

async fn lane_worker(ctx: LaneContext) -> Result<(), TransferError> {
    let handle = match ctx.direction {
        TransferDirection::Upload => Some(open_upload_part(&ctx.session, &ctx.remote_part).await?),
        TransferDirection::Download => {
            Some(open_download_source(&ctx.session, &ctx.remote_source).await?)
        }
    };

    let mut local_reader = if ctx.direction == TransferDirection::Upload {
        Some(
            tokio::fs::File::open(&ctx.local_source)
                .await
                .map_err(|err| {
                    failed(format!(
                        "Failed to open local source '{}': {err}",
                        ctx.local_source.display()
                    ))
                })?,
        )
    } else {
        None
    };

    let mut local_writer = if ctx.direction == TransferDirection::Download {
        Some(
            tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&ctx.local_part)
                .await
                .map_err(|err| {
                    failed(format!(
                        "Failed to open local part '{}': {err}",
                        ctx.local_part.display()
                    ))
                })?,
        )
    } else {
        None
    };

    let mut last_progress = ctx.transferred.load(Ordering::Relaxed);

    let result = loop {
        let index = {
            let mut queue = ctx.queue.lock().await;
            queue.pop_front()
        };
        let Some(index) = index else { break Ok(()) };

        if *ctx.cancel.borrow() {
            break Err(TransferError::Cancelled);
        }

        let offset = index as u64 * ctx.chunk_size;
        let len = chunk_len_for(index, ctx.chunk_size, ctx.total_size);

        match ctx.direction {
            TransferDirection::Upload => {
                let reader = local_reader.as_mut().expect("upload reader");
                let mut buffer = vec![0u8; len as usize];
                reader
                    .seek(std::io::SeekFrom::Start(offset))
                    .await
                    .map_err(|err| failed(format!("Failed to seek local source: {err}")))?;
                reader.read_exact(&mut buffer).await.map_err(|err| {
                    failed(format!(
                        "Failed to read local source at offset {offset}: {err}"
                    ))
                })?;
                let handle = handle.as_ref().expect("upload handle");
                write_remote_chunk(&ctx.session, handle, offset, &buffer, ctx.write_limit).await?;
            }
            TransferDirection::Download => {
                let handle = handle.as_ref().expect("download handle");
                let data = read_remote_chunk(
                    &ctx.session,
                    handle,
                    offset,
                    len,
                    ctx.read_limit.unwrap_or(DEFAULT_IO_STEP),
                )
                .await?;
                if data.len() as u64 != len {
                    break Err(failed(format!(
                        "Short read from remote source at offset {offset}: got {} of {len} bytes",
                        data.len()
                    )));
                }
                let writer = local_writer.as_mut().expect("download writer");
                writer
                    .seek(std::io::SeekFrom::Start(offset))
                    .await
                    .map_err(|err| failed(format!("Failed to seek local part: {err}")))?;
                writer
                    .write_all(&data)
                    .await
                    .map_err(|err| failed(format!("Failed to write local part: {err}")))?;
                writer
                    .flush()
                    .await
                    .map_err(|err| failed(format!("Failed to flush local part: {err}")))?;
            }
        }

        // Persist the checkpoint only after the chunk data is safely in place.
        {
            let mut state = ctx.sidecar_state.lock().await;
            if !bitmap_get(&state.bits, index) {
                bitmap_set(&mut state.bits, index);
                state.sidecar.completed = encode_bitmap(&state.bits);
                state.sidecar.updated_at_ms = now_unix_ms();
                ctx.sidecar_store.save(&state.sidecar).await?;
                ctx.completed_chunks.fetch_add(1, Ordering::SeqCst);
            }
        }

        let total = ctx.transferred.fetch_add(len, Ordering::SeqCst) + len;
        if total.saturating_sub(last_progress) >= ctx.progress_interval || total >= ctx.total_size {
            last_progress = total;
            (ctx.progress)(TransferProgress {
                transferred: total,
                total: ctx.total_size,
                resumed_from: ctx.resumed_from,
                parallelism: ctx.parallelism,
                completed_chunks: ctx.completed_chunks.load(Ordering::SeqCst),
                chunk_count: ctx.chunk_count as u64,
            });
        }
    };

    if let Some(handle) = handle {
        let _ = ctx.session.close(handle.as_str()).await;
    }

    result
}

async fn open_upload_part(session: &RawSftpSession, path: &str) -> Result<String, TransferError> {
    session
        .open(
            path,
            OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::READ,
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

async fn write_remote_chunk(
    session: &RawSftpSession,
    handle: &str,
    offset: u64,
    data: &[u8],
    limit: Option<u64>,
) -> Result<(), TransferError> {
    let step = limit
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_IO_STEP)
        .clamp(1, u32::MAX as u64) as usize;
    let mut written = 0usize;
    while written < data.len() {
        let end = (written + step).min(data.len());
        session
            .write(handle, offset + written as u64, data[written..end].to_vec())
            .await
            .map_err(|err| failed(format!("Failed to write remote part: {err}")))?;
        written = end;
    }
    Ok(())
}

async fn read_remote_chunk(
    session: &RawSftpSession,
    handle: &str,
    offset: u64,
    len: u64,
    limit: u64,
) -> Result<Vec<u8>, TransferError> {
    let step = limit.clamp(1, u32::MAX as u64);
    let mut buffer = Vec::with_capacity(len as usize);
    while (buffer.len() as u64) < len {
        let remaining = len - buffer.len() as u64;
        let request = remaining.min(step) as u32;
        let data = session
            .read(handle, offset + buffer.len() as u64, request)
            .await
            .map_err(|err| failed(format!("Failed to read remote source: {err}")))?;
        if data.data.is_empty() {
            break;
        }
        buffer.extend_from_slice(&data.data);
    }
    Ok(buffer)
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
