//! The parallel lanes of a transfer: each one pipelines chunk requests over
//! its own SFTP session.

use super::checkpoint::{checkpoint_due, CheckpointSnapshot, CheckpointWriter, SidecarState};
use super::chunks::{
    bitmap_count, bitmap_get, bitmap_set, encode_bitmap, io_read_step, io_write_step,
};
use super::files::now_unix_ms;
use super::run::{TransferPaths, TransferRun};
use super::{
    chunk_len_for, failed, rate_limit, ProgressSink, RemoteChannels, TransferDirection,
    TransferError, TransferOptions, TransferProgress, LANE_CLOSE_TIMEOUT,
};
use russh_sftp::client::RawSftpSession;
use russh_sftp::protocol::{FileAttributes, OpenFlags};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinSet;

pub(super) async fn execute_parallel(
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
        last_progress_at: std::sync::Mutex::new(None),
        completed_chunks: Arc::new(AtomicU64::new(run.initial_completed_chunks)),
        progress: progress.clone(),
        cancel: cancel.clone(),
        chunk_size: run.chunk_size,
        total_size: run.total_size,
        chunk_count: run.chunk_count,
        resumed_from: run.resumed_from,
        parallelism: options.parallelism,
        progress_interval: options.progress_interval,
        sidecar_state: run.sidecar_state.clone(),
        local_source: paths.local_path.clone(),
        local_part: PathBuf::from(&paths.part_path),
        remote_part: paths.part_path.clone(),
        remote_source: paths.remote_path.clone(),
        direction: run.direction,
        read_step: io_read_step(channels.limits.as_ref()),
        write_step: io_write_step(channels.limits.as_ref()),
        checkpointing: run.checkpointing,
        pipeline_window: run.pipeline_window,
        checkpoint_tx: checkpoint.and_then(|writer| writer.sender()),
        rate_limiter: options.rate_limiter.clone(),
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
    /// When progress was last reported; held while reporting so reports
    /// from different lanes go out one at a time.
    last_progress_at: std::sync::Mutex<Option<Instant>>,
    completed_chunks: Arc<AtomicU64>,
    progress: ProgressSink,
    cancel: watch::Receiver<bool>,
    chunk_size: u64,
    total_size: u64,
    chunk_count: usize,
    resumed_from: u64,
    parallelism: usize,
    progress_interval: Duration,
    sidecar_state: Arc<Mutex<SidecarState>>,
    local_source: PathBuf,
    local_part: PathBuf,
    remote_part: String,
    remote_source: String,
    direction: TransferDirection,
    read_step: u64,
    write_step: u64,
    checkpointing: bool,
    pipeline_window: usize,
    checkpoint_tx: Option<mpsc::Sender<CheckpointSnapshot>>,
    rate_limiter: Option<Arc<rate_limit::RateLimiter>>,
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
    // With `pipeline_window == 1` the lane is strictly sequential: one chunk,
    // and therefore one write request, at a time in ascending offset order.
    let mut chunk_tasks: JoinSet<(usize, Result<(), TransferError>)> = JoinSet::new();

    let result: Result<(), TransferError> = loop {
        while chunk_tasks.len() < shared.pipeline_window {
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
                record_chunk_completion(&shared, index).await;
            }
            Ok((_, Err(err))) => break Err(err),
            Err(join_err) => break Err(failed(format!("Transfer worker failed: {join_err}"))),
        }
    };

    chunk_tasks.abort_all();
    while chunk_tasks.join_next().await.is_some() {}
    // Closing the remote part is a courtesy on a healthy link, but on the
    // failure path the lane is stopping precisely because the session stopped
    // answering: waiting a full request timeout for that CLOSE would add
    // another ten seconds before the user sees the error.
    let close = session.close(handle.as_str());
    if result.is_err() {
        let _ = tokio::time::timeout(LANE_CLOSE_TIMEOUT, close).await;
    } else {
        let _ = close.await;
    }

    result
}

fn record_transfer_progress(shared: &LaneShared, step_bytes: u64) {
    if step_bytes == 0 {
        return;
    }
    let total = shared.transferred.fetch_add(step_bytes, Ordering::SeqCst) + step_bytes;
    report_progress(shared, total, total >= shared.total_size);
}

/// Reports progress at most once per `progress_interval`, except for the
/// final report, which always goes out so the transfer reaches 100%.
fn report_progress(shared: &LaneShared, total: u64, finished: bool) {
    let mut last_progress_at = shared
        .last_progress_at
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    if !finished
        && last_progress_at.is_some_and(|at| now.duration_since(at) < shared.progress_interval)
    {
        return;
    }
    *last_progress_at = Some(now);
    (shared.progress)(TransferProgress {
        transferred: total.min(shared.total_size),
        total: shared.total_size,
        resumed_from: shared.resumed_from,
        parallelism: shared.parallelism,
        completed_chunks: shared.completed_chunks.load(Ordering::SeqCst),
        chunk_count: shared.chunk_count as u64,
    });
}

/// Move one acknowledged chunk into the bitmap (throttled checkpoint) and
/// emit progress. The chunk data is already durable at this point, so a
/// failed checkpoint save is deliberately non-fatal: the next cadence save
/// or the cancel/error flush will retry it.
async fn record_chunk_completion(shared: &LaneShared, index: usize) {
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

    let total = shared.transferred.load(Ordering::SeqCst);
    let finished = total >= shared.total_size
        || shared.completed_chunks.load(Ordering::SeqCst) >= shared.chunk_count as u64;
    report_progress(shared, total, finished);
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

/// Largest request the bandwidth limit allows right now; unbounded without one.
fn max_throttled_request(shared: &LaneShared) -> u64 {
    shared
        .rate_limiter
        .as_ref()
        .and_then(|limiter| limiter.max_request())
        .map_or(u64::MAX, |max| max.max(1))
}

/// Waits for the bandwidth budget of one request, if a limit is set.
async fn throttle(shared: &LaneShared, bytes: u64) -> Result<(), TransferError> {
    match shared.rate_limiter.as_ref() {
        Some(limiter) if !limiter.acquire(bytes, &shared.cancel).await => {
            Err(TransferError::Cancelled)
        }
        _ => Ok(()),
    }
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
        let want = (len - pos)
            .min(shared.write_step)
            .min(max_throttled_request(shared)) as usize;
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
        throttle(shared, want as u64).await?;
        session
            .write(handle, offset + pos, buffer[..want].to_vec())
            .await
            .map_err(|err| failed(format!("Failed to write remote part: {err}")))?;
        pos += want as u64;
        let is_last = pos == len;
        if is_last {
            shared.transferred.fetch_add(want as u64, Ordering::SeqCst);
        } else {
            record_transfer_progress(shared, want as u64);
        }
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
        let request = (len - pos).min(step).min(max_throttled_request(shared)) as u32;
        throttle(shared, u64::from(request)).await?;
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
        let read_len = data.data.len() as u64;
        pos += read_len;
        let is_last = pos == len;
        if is_last {
            shared.transferred.fetch_add(read_len, Ordering::SeqCst);
        } else {
            record_transfer_progress(shared, read_len);
        }
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
