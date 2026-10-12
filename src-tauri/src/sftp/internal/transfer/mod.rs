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

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use russh_sftp::client::RawSftpSession;
use russh_sftp::extensions::LimitsExtension;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use run::run_transfer;

mod checkpoint;
mod chunks;
mod files;
mod lanes;
pub mod rate_limit;
mod run;
#[cfg(test)]
pub(crate) mod test_server;
#[cfg(test)]
mod tests;

pub const SIDECAR_VERSION: u32 = 1;
pub const SIDECAR_SUFFIX: &str = ".tterm.part.json";
pub const PART_SUFFIX: &str = ".tterm.part";
pub const DEFAULT_CHUNK_SIZE: u64 = 4 * 1024 * 1024;
pub const DEFAULT_PARALLELISM: usize = 4;
/// Minimum gap between progress reports of one transfer. A step finishes
/// every couple of milliseconds on a fast link, and each report is serialized
/// and sent to the webview, which only redraws a few times a second anyway.
pub const DEFAULT_PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
const MIN_CHUNK_SIZE: u64 = 4 * 1024;
/// Fallback step for a single SFTP read/write when the server does not
/// advertise `limits@openssh.com` (mirrors russh-sftp's own default).
///
/// 255 KiB, not 256 KiB: OpenSSH caps a whole SFTP message at 256 KiB, and a
/// WRITE carries ~25 bytes of framing (type, request id, handle, offset and
/// two length prefixes) on top of its payload. A 256 KiB payload overflows
/// that cap, and `sftp-server` answers by killing the subsystem — so on a
/// server predating the `limits@openssh.com` extension (OpenSSH < 8.5) not a
/// single write is ever acknowledged and the transfer sits at 0%.
const DEFAULT_IO_STEP: u64 = 255 * 1024;
pub const MAX_PARALLELISM: usize = 16;
/// Chunk requests one lane keeps in flight. SFTP multiplexes requests by id,
/// so a lane does not have to wait for each acknowledgement before sending
/// the next one; real servers (OpenSSH) process them concurrently.
///
/// This is an upper bound, not a fixed value: a server whose storage cannot
/// keep up with several concurrent writers answers none of them within the
/// client's request timeout, which surfaces as `Failed to write ...: Timeout`
/// with zero progress. A single lane is therefore run strictly sequentially
/// (see [`TransferOptions::normalized`]).
pub const PIPELINE_WINDOW: usize = 8;
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
/// Upper bound on the post-failure checkpoint teardown. The connection that
/// just failed is typically the one that would carry those writes, so the
/// flush is given a generous-but-finite budget instead of the full
/// per-request timeout for every queued snapshot.
const CHECKPOINT_TEARDOWN_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a failing lane waits for its final remote-close before giving up
/// on it, so a dead session cannot stretch the failure by another full
/// per-request timeout.
const LANE_CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

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
    /// Minimum gap between progress reports; the final one always goes out.
    pub progress_interval: Duration,
    /// Chunk requests one lane keeps in flight (clamped to
    /// [`PIPELINE_WINDOW`]). `1` makes a lane strictly sequential — one
    /// request at a time, in ascending offset order, the way plain
    /// `sftp`/`scp` write.
    pub pipeline_window: usize,
    /// Bandwidth budget this transfer draws from; `None` is unlimited. Shared
    /// across transfers so a limit caps their sum, not each of them.
    pub rate_limiter: Option<Arc<rate_limit::RateLimiter>>,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            parallelism: DEFAULT_PARALLELISM,
            chunk_size: DEFAULT_CHUNK_SIZE,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            pipeline_window: PIPELINE_WINDOW,
            rate_limiter: None,
        }
    }
}

impl TransferOptions {
    pub fn normalized(mut self) -> Self {
        self.parallelism = self.parallelism.clamp(1, MAX_PARALLELISM);
        self.chunk_size = self.chunk_size.max(MIN_CHUNK_SIZE);
        self.pipeline_window = self.pipeline_window.clamp(1, PIPELINE_WINDOW);
        // A single lane is the compatibility mode: one chunk request in
        // flight, so no two writes at different offsets are ever outstanding
        // at the same time. This is what a fragile server needs — with the
        // usual window its storage can stall long enough that every in-flight
        // request hits the client's request timeout and no chunk is ever
        // acknowledged.
        if self.parallelism == 1 {
            self.pipeline_window = 1;
        }
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
