//! One transfer from start to finish: resuming from the checkpoint, running
//! the lanes, and renaming the part file onto the destination.

use super::checkpoint::{CheckpointWriter, SidecarState, SidecarStore};
use super::chunks::{
    bitmap_count, bitmap_get, bitmap_len, completed_bytes, decode_bitmap, effective_chunk_size,
    encode_bitmap, io_read_step, io_write_step, required_part_size, step_from_limits,
};
use super::files::{ensure_remote_dir_all, local_fingerprint, now_unix_ms, remote_fingerprint};
use super::lanes::execute_parallel;
use super::{
    chunk_count_for, chunk_len_for, failed, part_path, sidecar_path, ProgressSink, RemoteChannels,
    TransferDirection, TransferError, TransferOptions, TransferOutcome, TransferProgress,
    TransferSidecar, CHECKPOINT_TEARDOWN_TIMEOUT, PIPELINE_WINDOW, SIDECAR_SUFFIX, SIDECAR_VERSION,
};
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::RawSftpSession;
use russh_sftp::extensions::LimitsExtension;
use russh_sftp::protocol::{FileAttributes, OpenFlags, StatusCode};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{watch, Mutex};

pub(super) struct TransferPaths {
    pub(super) final_path: String,
    pub(super) part_path: String,
    pub(super) sidecar_store: Arc<SidecarStore>,
    /// Local source path (upload) or local destination path (download).
    pub(super) local_path: PathBuf,
    /// Remote destination path (upload) or remote source path (download).
    pub(super) remote_path: String,
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

#[expect(
    clippy::too_many_lines,
    reason = "predates the 150-line limit; split it when it next changes"
)]
pub(super) async fn run_transfer(
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
        pipeline_window: options.pipeline_window,
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
        //
        // Both steps are bounded: the connection that just failed is usually
        // the reason the transfer stopped (a stalled server stops answering
        // checkpoint writes too), so persisting through it must never turn a
        // fast failure into another minute of waiting. A dropped snapshot only
        // re-sends those chunks on the next attempt, it cannot corrupt anything.
        if let Some(writer) = checkpoint.as_ref() {
            let _ =
                tokio::time::timeout(CHECKPOINT_TEARDOWN_TIMEOUT, flush_checkpoint(&run, writer))
                    .await;
        }
        if let Some(writer) = checkpoint {
            let _ = tokio::time::timeout(CHECKPOINT_TEARDOWN_TIMEOUT, writer.finish()).await;
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
    })
}

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

pub(super) struct TransferRun {
    pub(super) direction: TransferDirection,
    pub(super) chunks: Vec<usize>,
    pub(super) chunk_size: u64,
    pub(super) total_size: u64,
    pub(super) chunk_count: usize,
    pub(super) resumed_from: u64,
    pub(super) initial_completed_chunks: u64,
    pub(super) checkpointing: bool,
    pub(super) pipeline_window: usize,
    pub(super) sidecar_state: Arc<Mutex<SidecarState>>,
    pub(super) sidecar_store: Arc<SidecarStore>,
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
