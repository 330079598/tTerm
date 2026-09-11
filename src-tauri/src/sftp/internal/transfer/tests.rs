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
fn negotiate_chunk_size_respects_server_limits() {
    let limits = LimitsExtension {
        max_packet_len: 300 * 1024,
        max_read_len: 32 * 1024,
        max_write_len: 16 * 1024,
        max_open_handles: 0,
    };
    assert_eq!(
        negotiate_chunk_size(4 * 1024 * 1024, Some(&limits)),
        16 * 1024
    );
    assert_eq!(negotiate_chunk_size(8 * 1024, Some(&limits)), 8 * 1024);
    assert_eq!(negotiate_chunk_size(1024, None), MIN_CHUNK_SIZE);
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
async fn parallel_upload_outperforms_serial_with_injected_latency() {
    let latency = Duration::from_millis(3);
    let server = TestServer::start(ServerOptions {
        limits: None,
        latency,
        rename_overwrites: true,
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
