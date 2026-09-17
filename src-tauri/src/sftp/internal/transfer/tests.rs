//! Integration tests for the resumable parallel transfer engine, running
//! against the in-process SFTP server.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::watch;

use super::test_server::{ServerOptions, TestLimits, TestServer};
use super::*;

fn content(len: usize) -> Vec<u8> {
    (0..len)
        .map(|index| ((index * 31 + 7) % 251) as u8)
        .collect()
}

fn options(chunk_size: u64, parallelism: usize) -> TransferOptions {
    TransferOptions {
        parallelism,
        chunk_size,
        progress_interval_bytes: 0,
        pipeline_window: PIPELINE_WINDOW,
    }
}

fn noop_progress() -> ProgressSink {
    Arc::new(|_progress: TransferProgress| {})
}

fn server_limits() -> TestLimits {
    TestLimits {
        max_packet_len: 256 * 1024,
        max_read_len: 32 * 1024,
        max_write_len: 32 * 1024,
    }
}

async fn write_local(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.unwrap();
    }
    tokio::fs::write(path, bytes).await.unwrap();
}

fn write_file_blocking(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
}

#[allow(clippy::too_many_arguments)]
fn build_sidecar(
    direction: TransferDirection,
    final_path: &str,
    part_path: &str,
    chunk_size: u64,
    total_size: u64,
    source_size: u64,
    source_mtime: Option<i64>,
    bits: &[u8],
) -> TransferSidecar {
    TransferSidecar {
        version: SIDECAR_VERSION,
        direction,
        final_path: final_path.to_string(),
        part_path: part_path.to_string(),
        chunk_size,
        total_size,
        source_size,
        source_mtime,
        completed: encode_bitmap(bits),
        updated_at_ms: now_unix_ms(),
    }
}

fn write_sidecar_json(path: &Path, sidecar: &TransferSidecar) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, serde_json::to_vec(sidecar).unwrap()).unwrap();
}

fn write_remote_partial(root: &Path, part: &str, content: &[u8], bits: &[u8], chunk_size: u64) {
    let part_path = root.join(part);
    if let Some(parent) = part_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&part_path)
        .unwrap();
    use std::io::{Seek, SeekFrom, Write};
    for (index, byte) in bits.iter().enumerate() {
        for bit in 0..8 {
            if byte & (1 << bit) == 0 {
                continue;
            }
            let chunk = index * 8 + bit;
            let start = chunk as u64 * chunk_size;
            if start >= content.len() as u64 {
                continue;
            }
            let end = ((start + chunk_size) as usize).min(content.len());
            file.seek(SeekFrom::Start(start)).unwrap();
            file.write_all(&content[start as usize..end]).unwrap();
        }
    }
    file.flush().unwrap();
}

#[test]
fn fallback_io_step_fits_openssh_max_message() {
    // OpenSSH caps a whole SFTP message at 256 KiB and kills the subsystem
    // when one exceeds it. A WRITE adds framing on top of the payload:
    // type(1) + request id(4) + handle(4 + len) + offset(8) + data length(4).
    // A server that predates `limits@openssh.com` (OpenSSH < 8.5) advertises
    // nothing, so this fallback is what goes on the wire — and if it does not
    // leave room for the framing, no write is ever acknowledged and the
    // transfer stalls at 0%.
    const OPENSSH_MAX_MSG_LENGTH: u64 = 256 * 1024;
    let framing = 1 + 4 + (4 + 4) + 8 + 4;

    for step in [io_write_step(None), io_read_step(None)] {
        assert!(
            step + framing <= OPENSSH_MAX_MSG_LENGTH,
            "fallback step {step} + {framing} bytes of framing exceeds OpenSSH's {OPENSSH_MAX_MSG_LENGTH} byte message cap"
        );
    }
}

#[test]
fn chunk_size_is_not_clamped_by_wire_limits() {
    // The chunk is the logical resume unit: a server advertising small
    // read/write limits (e.g. OpenSSH's ~256 KiB) must not shrink it — the
    // per-request steps (`io_write_step`/`io_read_step`) absorb the wire
    // limit instead, so large files keep 4 MiB chunks instead of tens of
    // thousands of wire-sized ones.
    let limits = LimitsExtension {
        max_packet_len: 300 * 1024,
        max_read_len: 32 * 1024,
        max_write_len: 16 * 1024,
        max_open_handles: 0,
    };
    let step = io_write_step(Some(&limits));
    assert_eq!(step, 16 * 1024);
    assert_eq!(
        effective_chunk_size(4 * 1024 * 1024, step, 512 * 1024 * 1024, 8),
        4 * 1024 * 1024
    );
}

#[test]
fn effective_chunk_size_fills_window_for_small_files() {
    let mib = 1024 * 1024u64;
    // No-limits server, 3 MiB file: 8 × 384 KiB chunks instead of one
    // 4 MiB chunk whose steps would serialize.
    assert_eq!(
        effective_chunk_size(4 * mib, 256 * 1024, 3 * mib, 8),
        384 * 1024
    );
    // Large transfer: the negotiated chunk is untouched.
    assert_eq!(
        effective_chunk_size(4 * mib, 256 * 1024, 5 * 1024 * mib, 8),
        4 * mib
    );
    // File smaller than one write step: a single (whole-file) chunk.
    assert_eq!(
        effective_chunk_size(4 * mib, 256 * 1024, 200 * 1024, 8),
        256 * 1024
    );
    // Already-small negotiated chunks (test shape: 8 KiB chunks, 32 KiB
    // step) stay untouched, as do chunks at or below their step.
    assert_eq!(
        effective_chunk_size(8 * 1024, 32 * 1024, 40 * 1024, 8),
        8 * 1024
    );
    assert_eq!(effective_chunk_size(261_120, 262_144, 3 * mib, 8), 261_120);
    // Empty transfer: nothing to pipeline.
    assert_eq!(effective_chunk_size(4 * mib, 256 * 1024, 0, 8), 4 * mib);
}

#[test]
fn bitmap_round_trips_and_rehomes() {
    let bits = vec![0b1010_0101u8, 0b0000_0011];
    let encoded = encode_bitmap(&bits);
    let decoded = decode_bitmap(&encoded, 11).unwrap();
    assert_eq!(decoded, bits);

    // A short bitmap is padded, a long one truncated.
    assert_eq!(decode_bitmap(&encode_bitmap(&[0xFF]), 20).unwrap().len(), 3);
}

#[test]
fn checkpoint_cadence_ignores_chunk_counts_below_byte_threshold() {
    let mut state = SidecarState {
        sidecar: build_sidecar(TransferDirection::Upload, "f", "p", 4096, 0, 0, None, &[]),
        bits: Vec::new(),
        unsaved_chunks: 8,
        unsaved_bytes: 8 * 4096,
        persisted_chunks: 0,
        last_checkpoint: Instant::now(),
    };
    // A handful of wire-sized chunks (a few KiB each) must not trigger a
    // save: chunk counts alone would flood the store with round trips.
    assert!(!checkpoint_due(&state));
    // The interval backstop bounds the re-send window on slow links.
    state.last_checkpoint = Instant::now() - CHECKPOINT_MIN_INTERVAL - Duration::from_millis(1);
    assert!(checkpoint_due(&state));
    // The byte threshold fires regardless of the interval.
    state.last_checkpoint = Instant::now();
    state.unsaved_bytes = CHECKPOINT_MIN_BYTES;
    assert!(checkpoint_due(&state));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn upload_fills_bitmap_holes_and_is_byte_identical() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        ..Default::default()
    })
    .await;
    let local_dir = server.local_dir().to_path_buf();
    let chunk_size = 8 * 1024u64;
    let data = content(chunk_size as usize * 5);
    let local_path = local_dir.join("holes-source.bin");
    write_local(&local_path, &data).await;

    let relative = "holes.bin";
    let part = part_path(relative);
    let sidecar_path = sidecar_path(relative);

    // Chunks 0 and 2 present, chunk 1 and 3..4 missing (a real hole).
    let mut bits = vec![0u8; bitmap_len(5)];
    bitmap_set(&mut bits, 0);
    bitmap_set(&mut bits, 2);
    write_remote_partial(server.root(), &part, &data, &bits, chunk_size);

    let (_, mtime) = local_fingerprint(&local_path).await.unwrap();
    let sidecar = build_sidecar(
        TransferDirection::Upload,
        relative,
        &part,
        chunk_size,
        data.len() as u64,
        data.len() as u64,
        mtime,
        &bits,
    );
    write_sidecar_json(&server.root().join(&sidecar_path), &sidecar);

    let outcome = upload_file(
        server.channels(4).await,
        local_path.clone(),
        relative.to_string(),
        options(chunk_size, 4),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(outcome.resumed_from, chunk_size * 2);
    assert_eq!(outcome.transferred, data.len() as u64);
    let final_bytes = std::fs::read(server.root().join(relative)).unwrap();
    assert_eq!(final_bytes, data);
    assert!(!server.root().join(&part).exists());
    assert!(!server.root().join(&sidecar_path).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn upload_resume_from_partial_remote_part() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        ..Default::default()
    })
    .await;
    let local_dir = server.local_dir().to_path_buf();
    let chunk_size = 8 * 1024u64;
    let data = content(chunk_size as usize * 6);
    let local_path = local_dir.join("partial-source.bin");
    write_local(&local_path, &data).await;

    let relative = "partial.bin";
    let (cancel_tx, rx) = watch::channel(false);
    let threshold = chunk_size * 3;
    let progress: ProgressSink = Arc::new(move |update: TransferProgress| {
        if update.resumed_from == 0 && update.transferred >= threshold {
            let _ = cancel_tx.send(true);
        }
    });

    let first = upload_file(
        server.channels(1).await,
        local_path.clone(),
        relative.to_string(),
        options(chunk_size, 1),
        rx,
        progress,
    )
    .await;
    assert!(matches!(first, Err(TransferError::Cancelled)));

    let part = part_path(relative);
    assert!(server.root().join(&part).exists(), "part must be preserved");
    assert!(
        server.root().join(sidecar_path(relative)).exists(),
        "sidecar must be preserved"
    );

    let outcome = upload_file(
        server.channels(4).await,
        local_path.clone(),
        relative.to_string(),
        options(chunk_size, 4),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(outcome.resumed_from, threshold);
    let final_bytes = std::fs::read(server.root().join(relative)).unwrap();
    assert_eq!(final_bytes, data);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn download_resume_from_partial_local_part() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        ..Default::default()
    })
    .await;
    let local_dir = server.local_dir().to_path_buf();
    let chunk_size = 8 * 1024u64;
    let data = content(chunk_size as usize * 6);
    let relative = "download-source.bin";
    write_file_blocking(&server.root().join(relative), &data);

    let local_path = local_dir.join("download-target.bin");
    let part = PathBuf::from(part_path(&local_path.to_string_lossy()));
    let sidecar_path = PathBuf::from(format!("{}{SIDECAR_SUFFIX}", local_path.to_string_lossy()));

    // Pre-populate the local part with the first two chunks.
    write_file_blocking(&part, &data[..chunk_size as usize * 2]);

    let (size, mtime) = remote_fingerprint(server.channels(1).await.sessions[0].as_ref(), relative)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(size, data.len() as u64);
    let mut bits = vec![0u8; bitmap_len(6)];
    bitmap_set(&mut bits, 0);
    bitmap_set(&mut bits, 1);
    let sidecar = build_sidecar(
        TransferDirection::Download,
        &local_path.to_string_lossy(),
        &part.to_string_lossy(),
        chunk_size,
        data.len() as u64,
        data.len() as u64,
        mtime,
        &bits,
    );
    write_sidecar_json(&sidecar_path, &sidecar);

    let outcome = download_file(
        server.channels(4).await,
        local_path.clone(),
        relative.to_string(),
        options(chunk_size, 4),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(outcome.resumed_from, chunk_size * 2);
    let final_bytes = std::fs::read(&local_path).unwrap();
    assert_eq!(final_bytes, data);
    assert!(!part.exists());
    assert!(!sidecar_path.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn upload_mtime_or_size_mismatch_retransmits() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        ..Default::default()
    })
    .await;
    let local_dir = server.local_dir().to_path_buf();
    let chunk_size = 8 * 1024u64;
    let data = content(chunk_size as usize * 4);
    let local_path = local_dir.join("mismatch-source.bin");
    write_local(&local_path, &data).await;

    for (label, fingerprint_size, fingerprint_mtime) in [
        ("size", data.len() as u64 + 1, None),
        ("mtime", data.len() as u64, Some(1_234_567)),
    ] {
        let relative = format!("mismatch-{label}.bin");
        let part = part_path(&relative);
        let sidecar_path = sidecar_path(&relative);

        // Garbage part + a checkpoint that claims every chunk is done.
        let garbage = vec![0xABu8; data.len()];
        write_file_blocking(&server.root().join(&part), &garbage);
        let mut bits = vec![0u8; bitmap_len(4)];
        for index in 0..4 {
            bitmap_set(&mut bits, index);
        }
        let (_, actual_mtime) = local_fingerprint(&local_path).await.unwrap();
        let sidecar = build_sidecar(
            TransferDirection::Upload,
            &relative,
            &part,
            chunk_size,
            fingerprint_size,
            fingerprint_size,
            if label == "mtime" {
                fingerprint_mtime
            } else {
                actual_mtime
            },
            &bits,
        );
        write_sidecar_json(&server.root().join(&sidecar_path), &sidecar);

        let outcome = upload_file(
            server.channels(4).await,
            local_path.clone(),
            relative.clone(),
            options(chunk_size, 4),
            watch::channel(false).1,
            noop_progress(),
        )
        .await
        .unwrap();

        assert_eq!(outcome.resumed_from, 0, "{label} mismatch must restart");
        assert_eq!(
            std::fs::read(server.root().join(&relative)).unwrap(),
            data,
            "{label} mismatch must not keep unrelated bytes"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn complete_checkpoint_with_missing_part_retransmits() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        ..Default::default()
    })
    .await;
    let local_path = server.local_dir().join("missing-part-source.bin");
    let chunk_size = 8 * 1024u64;
    let data = content(chunk_size as usize * 3);
    write_local(&local_path, &data).await;

    let relative = "missing-part.bin";
    let part = part_path(relative);
    let sidecar_path = sidecar_path(relative);

    // Checkpoint claims every chunk is done, but the part file was lost.
    let mut bits = vec![0u8; bitmap_len(3)];
    for index in 0..3 {
        bitmap_set(&mut bits, index);
    }
    let (_, mtime) = local_fingerprint(&local_path).await.unwrap();
    let sidecar = build_sidecar(
        TransferDirection::Upload,
        relative,
        &part,
        chunk_size,
        data.len() as u64,
        data.len() as u64,
        mtime,
        &bits,
    );
    write_sidecar_json(&server.root().join(&sidecar_path), &sidecar);

    let outcome = upload_file(
        server.channels(2).await,
        local_path.clone(),
        relative.to_string(),
        options(chunk_size, 2),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(outcome.resumed_from, 0, "a missing part must be rebuilt");
    assert_eq!(std::fs::read(server.root().join(relative)).unwrap(), data);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn partial_checkpoint_with_missing_part_retransmits() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        ..Default::default()
    })
    .await;
    let local_path = server.local_dir().join("holes-missing-part-source.bin");
    let chunk_size = 8 * 1024u64;
    let data = content(chunk_size as usize * 5);
    write_local(&local_path, &data).await;

    let relative = "holes-missing-part.bin";
    let part = part_path(relative);
    let sidecar_path = sidecar_path(relative);

    // Checkpoint claims chunks 0 and 2 are done but the part file was lost
    // out-of-band. Resuming into a fresh part file would stitch zero-filled
    // holes in front of the renamed final file, so the transfer must
    // restart from scratch.
    let mut bits = vec![0u8; bitmap_len(5)];
    bitmap_set(&mut bits, 0);
    bitmap_set(&mut bits, 2);
    let (_, mtime) = local_fingerprint(&local_path).await.unwrap();
    let sidecar = build_sidecar(
        TransferDirection::Upload,
        relative,
        &part,
        chunk_size,
        data.len() as u64,
        data.len() as u64,
        mtime,
        &bits,
    );
    write_sidecar_json(&server.root().join(&sidecar_path), &sidecar);

    let outcome = upload_file(
        server.channels(2).await,
        local_path.clone(),
        relative.to_string(),
        options(chunk_size, 2),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(
        outcome.resumed_from, 0,
        "a partial checkpoint without its part bytes must be discarded"
    );
    assert_eq!(
        std::fs::read(server.root().join(relative)).unwrap(),
        data,
        "the rebuilt file must be byte-identical"
    );
    assert!(!server.root().join(&part).exists());
    assert!(!server.root().join(&sidecar_path).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn remote_sidecar_tmp_fallback_recovers_checkpoint() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        ..Default::default()
    })
    .await;
    let local_dir = server.local_dir().to_path_buf();
    let chunk_size = 8 * 1024u64;
    let data = content(chunk_size as usize * 5);
    let local_path = local_dir.join("tmp-fallback-source.bin");
    write_local(&local_path, &data).await;

    let relative = "tmp-fallback.bin";
    let part = part_path(relative);
    let sidecar_path = sidecar_path(relative);

    // A crashed save swap (remove primary, crash before rename) leaves only
    // the temp copy: chunks 0 and 1 are done, and the checkpoint exists
    // solely as `<sidecar>.tmp`.
    let mut bits = vec![0u8; bitmap_len(5)];
    bitmap_set(&mut bits, 0);
    bitmap_set(&mut bits, 1);
    write_remote_partial(server.root(), &part, &data, &bits, chunk_size);
    let (_, mtime) = local_fingerprint(&local_path).await.unwrap();
    let sidecar = build_sidecar(
        TransferDirection::Upload,
        relative,
        &part,
        chunk_size,
        data.len() as u64,
        data.len() as u64,
        mtime,
        &bits,
    );
    write_file_blocking(
        &server.root().join(format!("{sidecar_path}.tmp")),
        &serde_json::to_vec(&sidecar).unwrap(),
    );

    let outcome = upload_file(
        server.channels(4).await,
        local_path.clone(),
        relative.to_string(),
        options(chunk_size, 4),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(
        outcome.resumed_from,
        chunk_size * 2,
        "the temp checkpoint must be recovered instead of discarding the transfer"
    );
    assert_eq!(std::fs::read(server.root().join(relative)).unwrap(), data);
    assert!(!server.root().join(&part).exists());
    assert!(!server.root().join(&sidecar_path).exists());
    assert!(!server.root().join(format!("{sidecar_path}.tmp")).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rename_overwrite_and_remove_then_rename_fallback() {
    let chunk_size = 8 * 1024u64;
    let data = content(chunk_size as usize * 3);

    for rename_overwrites in [true, false] {
        let server = TestServer::start(ServerOptions {
            limits: Some(server_limits()),
            rename_overwrites,
            ..Default::default()
        })
        .await;
        let local_path = server.local_dir().join("rename-source.bin");
        write_local(&local_path, &data).await;

        // A pre-existing destination must be replaced, not appended to.
        let relative = "rename-target.bin";
        write_file_blocking(&server.root().join(relative), b"stale existing content");

        let outcome = upload_file(
            server.channels(2).await,
            local_path.clone(),
            relative.to_string(),
            options(chunk_size, 2),
            watch::channel(false).1,
            noop_progress(),
        )
        .await
        .unwrap();

        assert_eq!(outcome.transferred, data.len() as u64);
        assert_eq!(std::fs::read(server.root().join(relative)).unwrap(), data);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancel_preserves_checkpoint_and_resumes_to_completion() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        ..Default::default()
    })
    .await;
    let local_dir = server.local_dir().to_path_buf();
    let chunk_size = 8 * 1024u64;
    let data = content(chunk_size as usize * 5);
    let local_path = local_dir.join("cancel-source.bin");
    write_local(&local_path, &data).await;

    let relative = "cancel-target.bin";
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let threshold = chunk_size * 2;
    let progress: ProgressSink = Arc::new(move |update: TransferProgress| {
        if update.resumed_from == 0 && update.transferred >= threshold {
            let _ = cancel_tx.send(true);
        }
    });

    let first = upload_file(
        server.channels(1).await,
        local_path.clone(),
        relative.to_string(),
        options(chunk_size, 1),
        cancel_rx,
        progress,
    )
    .await;
    assert!(matches!(first, Err(TransferError::Cancelled)));

    let part = part_path(relative);
    assert!(server.root().join(&part).exists());
    assert!(server.root().join(sidecar_path(relative)).exists());
    assert!(
        !server.root().join(relative).exists(),
        "final path must not appear before completion"
    );

    let outcome = upload_file(
        server.channels(4).await,
        local_path.clone(),
        relative.to_string(),
        options(chunk_size, 4),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(outcome.resumed_from, threshold);
    assert_eq!(std::fs::read(server.root().join(relative)).unwrap(), data);
    assert!(!server.root().join(&part).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn single_chunk_upload_skips_checkpoint_files() {
    let server = TestServer::start(ServerOptions::default()).await;
    let local_dir = server.local_dir().to_path_buf();
    // chunk_size larger than the file: exactly one chunk, checkpointing off.
    let data = content(16 * 1024);
    let local_path = local_dir.join("single-source.bin");
    write_local(&local_path, &data).await;

    let relative = "single.bin";
    let outcome = upload_file(
        server.channels(2).await,
        local_path.clone(),
        relative.to_string(),
        options(64 * 1024, 2),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(outcome.transferred, data.len() as u64);
    assert_eq!(std::fs::read(server.root().join(relative)).unwrap(), data);
    assert!(!server.root().join(part_path(relative)).exists());
    assert!(!server.root().join(sidecar_path(relative)).exists());
    assert!(!server
        .root()
        .join(format!("{}.tmp", sidecar_path(relative)))
        .exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn single_chunk_upload_replaces_stale_oversized_part() {
    let server = TestServer::start(ServerOptions::default()).await;
    let local_dir = server.local_dir().to_path_buf();
    let data = content(16 * 1024);
    let local_path = local_dir.join("stale-part-source.bin");
    write_local(&local_path, &data).await;

    let relative = "stale-part.bin";
    let part = part_path(relative);
    // A stale part from an older, larger attempt: a longer tail must not
    // survive the single-chunk rewrite.
    let stale: Vec<u8> = vec![0xABu8; 3 * data.len()];
    write_file_blocking(&server.root().join(&part), &stale);

    upload_file(
        server.channels(2).await,
        local_path.clone(),
        relative.to_string(),
        options(64 * 1024, 2),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(std::fs::read(server.root().join(relative)).unwrap(), data);
    assert!(!server.root().join(&part).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stale_part_survives_failing_remove_and_gets_truncated() {
    // Every remove is refused, so the fresh-transfer discard cannot delete
    // the stale oversized part: only the truncate fallback keeps the old
    // tail out of the renamed final file.
    let server = TestServer::start(ServerOptions {
        fail_removes: true,
        ..Default::default()
    })
    .await;
    let local_path = server.local_dir().join("remove-fail-source.bin");
    let data = content(16 * 1024);
    write_local(&local_path, &data).await;

    let relative = "remove-fail.bin";
    let part = part_path(relative);
    // A stale part from an older, larger attempt (4x the new size).
    write_file_blocking(&server.root().join(&part), &content(64 * 1024));

    upload_file(
        server.channels(2).await,
        local_path.clone(),
        relative.to_string(),
        options(64 * 1024, 2),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(
        std::fs::read(server.root().join(relative)).unwrap(),
        data,
        "the stale part tail must not survive the final rename"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn empty_file_upload_creates_empty_final() {
    let server = TestServer::start(ServerOptions::default()).await;
    let local_path = server.local_dir().join("empty-source.bin");
    write_local(&local_path, &[]).await;

    let relative = "empty.bin";
    upload_file(
        server.channels(2).await,
        local_path.clone(),
        relative.to_string(),
        options(8 * 1024, 2),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(
        std::fs::read(server.root().join(relative)).unwrap(),
        Vec::<u8>::new()
    );
    assert!(!server.root().join(part_path(relative)).exists());
    assert!(!server.root().join(sidecar_path(relative)).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn small_file_without_limits_shrinks_chunks_and_resumes() {
    // No limits advertised: the negotiated chunk would be the full 4 MiB
    // desired size, leaving a 1.5 MiB file as a single serial-stepped chunk.
    // The engine must shrink it to 256 KiB (one write step) so the lane
    // window pipelines the six chunks as six concurrent requests.
    let server = TestServer::start(ServerOptions::default()).await;
    let local_dir = server.local_dir().to_path_buf();
    let data = content(1536 * 1024);
    let local_path = local_dir.join("shrink-source.bin");
    write_local(&local_path, &data).await;

    let relative = "shrink-target.bin";
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let threshold = 256 * 1024 * 2;
    let progress: ProgressSink = Arc::new(move |update: TransferProgress| {
        if update.resumed_from == 0 && update.transferred >= threshold {
            let _ = cancel_tx.send(true);
        }
    });

    let first = upload_file(
        server.channels(2).await,
        local_path.clone(),
        relative.to_string(),
        options(4 * 1024 * 1024, 2),
        cancel_rx,
        progress,
    )
    .await;
    assert!(matches!(first, Err(TransferError::Cancelled)));

    let sidecar: TransferSidecar =
        serde_json::from_slice(&std::fs::read(server.root().join(sidecar_path(relative))).unwrap())
            .unwrap();
    assert_eq!(
        sidecar.chunk_size, DEFAULT_IO_STEP,
        "chunk must shrink to one write step"
    );
    assert_eq!(sidecar.total_size, data.len() as u64);
    assert!(
        sidecar.completed.len() > 2,
        "more than a single chunk must have been checkpointed"
    );

    let outcome = upload_file(
        server.channels(2).await,
        local_path.clone(),
        relative.to_string(),
        options(4 * 1024 * 1024, 2),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert!(outcome.resumed_from >= threshold);
    assert_eq!(std::fs::read(server.root().join(relative)).unwrap(), data);
    assert!(!server.root().join(part_path(relative)).exists());
    assert!(!server.root().join(sidecar_path(relative)).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn checkpoint_with_oversized_part_retransmits() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        ..Default::default()
    })
    .await;
    let local_path = server.local_dir().join("oversized-part-source.bin");
    let chunk_size = 8 * 1024u64;
    let data = content(chunk_size as usize * 3);
    write_local(&local_path, &data).await;

    let relative = "oversized-part.bin";
    let part = part_path(relative);
    let sidecar_path = sidecar_path(relative);

    // The checkpoint claims every chunk is done and the part file covers all
    // of them, but carries an extra stale tail from a larger older attempt.
    // Parts are opened without truncation, so resuming would rename that tail
    // into the final file as trailing garbage.
    let mut bits = vec![0u8; bitmap_len(3)];
    for index in 0..3 {
        bitmap_set(&mut bits, index);
    }
    let (_, mtime) = local_fingerprint(&local_path).await.unwrap();
    let sidecar = build_sidecar(
        TransferDirection::Upload,
        relative,
        &part,
        chunk_size,
        data.len() as u64,
        data.len() as u64,
        mtime,
        &bits,
    );
    write_sidecar_json(&server.root().join(&sidecar_path), &sidecar);
    let mut stale = data.clone();
    stale.extend_from_slice(&[0xCD; 4096]);
    write_file_blocking(&server.root().join(&part), &stale);

    let outcome = upload_file(
        server.channels(2).await,
        local_path.clone(),
        relative.to_string(),
        options(chunk_size, 2),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    assert_eq!(
        outcome.resumed_from, 0,
        "an oversized part must be discarded, not resumed"
    );
    assert_eq!(
        std::fs::read(server.root().join(relative)).unwrap(),
        data,
        "the retransmitted file must be byte-identical"
    );
    assert!(!server.root().join(&part).exists());
    assert!(!server.root().join(&sidecar_path).exists());
}

#[tokio::test]
async fn local_sidecar_removal_cleans_tmp_swap_file() {
    let dir = std::env::temp_dir().join("tterm-local-sidecar-remove-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("file.bin.tterm.part.json");
    let tmp = path.with_extension("json.tmp");
    write_file_blocking(&path, b"{}");
    write_file_blocking(&tmp, b"{}");

    SidecarStore::Local { path: path.clone() }.remove().await;

    assert!(!path.exists());
    assert!(
        !tmp.exists(),
        "the save() swap temp file must be cleaned up"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_upload_outperforms_serial_with_injected_latency() {
    let latency = Duration::from_millis(3);
    let server = TestServer::start(ServerOptions {
        limits: None,
        latency,
        rename_overwrites: true,
        fail_removes: false,
        stall_writes_after_bytes: None,
        recorded_write_offsets: None,
    })
    .await;
    let chunk_size = 16 * 1024u64;
    let chunks = 64usize;
    let data = content(chunk_size as usize * chunks);
    let local_path = server.local_dir().join("bench-source.bin");
    write_local(&local_path, &data).await;

    let serial_start = Instant::now();
    upload_file(
        server.channels(1).await,
        local_path.clone(),
        "bench-serial.bin".to_string(),
        options(chunk_size, 1),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();
    let serial = serial_start.elapsed();

    let parallel_start = Instant::now();
    upload_file(
        server.channels(4).await,
        local_path.clone(),
        "bench-parallel.bin".to_string(),
        options(chunk_size, 4),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();
    let parallel = parallel_start.elapsed();

    let serial_mib_s = (data.len() as f64 / 1024.0 / 1024.0) / serial.as_secs_f64();
    let parallel_mib_s = (data.len() as f64 / 1024.0 / 1024.0) / parallel.as_secs_f64();
    eprintln!(
        "transfer benchmark: serial={serial:?} ({serial_mib_s:.2} MiB/s), parallel={parallel:?} ({parallel_mib_s:.2} MiB/s)"
    );

    assert!(
        parallel < serial,
        "expected 4-way parallel ({parallel:?}) to beat serial ({serial:?})"
    );
    assert!(
        parallel.as_secs_f64() < serial.as_secs_f64() * 0.8,
        "expected a meaningful parallel speedup, serial={serial:?} parallel={parallel:?}"
    );
}

/// A remote that accepts bytes but silently stops acknowledging mid-transfer
/// must surface an error instead of wedging the transfer command forever.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stalled_remote_fails_instead_of_hanging() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        stall_writes_after_bytes: Some(256 * 1024),
        ..Default::default()
    })
    .await;

    let local_dir = server.local_dir().to_path_buf();
    let chunk_size = 4 * 1024 * 1024u64;
    let data = content(chunk_size as usize * 4);
    let local_path = local_dir.join("stall-source.bin");
    write_local(&local_path, &data).await;

    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(90),
        upload_file(
            server.channels(4).await,
            local_path,
            "stalled.bin".to_string(),
            options(chunk_size, 4),
            watch::channel(false).1,
            noop_progress(),
        ),
    )
    .await;

    match result {
        Err(_elapsed) => panic!(
            "upload hung on a stalled remote for {:?} instead of failing",
            started.elapsed()
        ),
        Ok(Ok(_)) => panic!("upload claimed success against a stalled remote"),
        Ok(Err(err)) => eprintln!("stalled upload failed after {:?}: {err}", started.elapsed()),
    }
}

/// A large upload that crosses the 32 MiB checkpoint byte-threshold with
/// OpenSSH-style limits, i.e. the exact branch a 274 MB upload takes, must
/// complete and stay byte-identical.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn large_upload_crossing_checkpoint_threshold_completes() {
    let server = TestServer::start(ServerOptions {
        limits: Some(server_limits()),
        ..Default::default()
    })
    .await;
    let local_dir = server.local_dir().to_path_buf();
    let chunk_size = 4 * 1024 * 1024u64;
    let data = content(64 * 1024 * 1024);
    let local_path = local_dir.join("large-source.bin");
    write_local(&local_path, &data).await;

    let started = Instant::now();
    let outcome = tokio::time::timeout(
        Duration::from_secs(120),
        upload_file(
            server.channels(4).await,
            local_path,
            "large.bin".to_string(),
            options(chunk_size, 4),
            watch::channel(false).1,
            noop_progress(),
        ),
    )
    .await
    .expect("large upload must finish")
    .expect("large upload must succeed");
    eprintln!(
        "64 MiB upload finished in {:?}, transferred {} bytes",
        started.elapsed(),
        outcome.transferred
    );

    let final_bytes = std::fs::read(server.root().join("large.bin")).unwrap();
    assert_eq!(final_bytes.len(), data.len());
    assert!(final_bytes == data, "large upload must be byte-identical");
}

#[test]
fn single_lane_normalizes_to_a_strictly_sequential_pipeline() {
    let sequential = options(4 * 1024 * 1024, 1).normalized();
    assert_eq!(sequential.parallelism, 1);
    assert_eq!(
        sequential.pipeline_window, 1,
        "one lane must keep exactly one chunk request in flight"
    );

    let parallel = options(4 * 1024 * 1024, 4).normalized();
    assert_eq!(parallel.pipeline_window, PIPELINE_WINDOW);

    let clamped = TransferOptions {
        pipeline_window: PIPELINE_WINDOW * 8,
        ..options(4 * 1024 * 1024, 4)
    }
    .normalized();
    assert_eq!(clamped.pipeline_window, PIPELINE_WINDOW);
}

/// With one lane the client must issue its writes strictly in ascending
/// offset order, the way plain `sftp`/`scp` do — that is what a server whose
/// storage cannot keep up with concurrent writers needs.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn single_lane_writes_in_ascending_offset_order() {
    let offsets = Arc::new(std::sync::Mutex::new(Vec::new()));
    let chunk_size = 8 * 1024u64;
    let server = TestServer::start(ServerOptions {
        limits: Some(TestLimits {
            max_packet_len: 256 * 1024,
            max_read_len: 32 * 1024,
            max_write_len: chunk_size,
        }),
        recorded_write_offsets: Some(offsets.clone()),
        ..Default::default()
    })
    .await;

    let data = content(chunk_size as usize * 8);
    let local_path = server.local_dir().join("sequential-source.bin");
    write_local(&local_path, &data).await;

    upload_file(
        server.channels(4).await,
        local_path,
        "sequential.bin".to_string(),
        options(chunk_size, 1),
        watch::channel(false).1,
        noop_progress(),
    )
    .await
    .unwrap();

    let recorded = offsets.lock().unwrap().clone();
    let expected: Vec<u64> = (0..8).map(|index| index * chunk_size).collect();
    assert_eq!(
        recorded, expected,
        "sequential mode must write exactly once per chunk, in ascending offset order"
    );
}

/// Progress must be reported as step writes complete rather than stalling at 0%
/// until an entire large chunk finishes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn step_level_progress_emits_before_chunk_completes() {
    let write_step = 16 * 1024u64;
    let chunk_size = 64 * 1024u64; // 4 steps per chunk
    let server = TestServer::start(ServerOptions {
        limits: Some(TestLimits {
            max_packet_len: 256 * 1024,
            max_read_len: 32 * 1024,
            max_write_len: write_step,
        }),
        ..Default::default()
    })
    .await;

    let data = content(chunk_size as usize); // Exactly 1 chunk
    let local_path = server.local_dir().join("step-progress-source.bin");
    write_local(&local_path, &data).await;

    let recorded_progress = Arc::new(std::sync::Mutex::new(Vec::new()));
    let progress_sink = {
        let recorded = recorded_progress.clone();
        Arc::new(move |update: TransferProgress| {
            recorded.lock().unwrap().push(update.transferred);
        })
    };

    let mut opts = options(chunk_size, 1);
    opts.progress_interval_bytes = 0; // Emit on every step

    upload_file(
        server.channels(1).await,
        local_path,
        "step-progress.bin".to_string(),
        opts,
        watch::channel(false).1,
        progress_sink,
    )
    .await
    .unwrap();

    let recorded = recorded_progress.lock().unwrap().clone();
    // Must have intermediate progress events before the final chunk completes
    assert!(
        recorded.iter().any(|&bytes| bytes > 0 && bytes < chunk_size),
        "expected step-level progress before full chunk completion, got {recorded:?}"
    );
    assert_eq!(*recorded.last().unwrap(), chunk_size);
}

