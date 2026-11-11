use super::*;
use crate::sftp::internal::transfer::{TransferSidecar, DEFAULT_PROGRESS_INTERVAL};

#[test]
fn default_progress_interval_preserves_first_sample_gate_and_forced_final() {
    let reports = Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = reports.clone();
    let (_, cancel) = watch::channel(false);
    let shared = LaneShared {
        queue: Arc::new(Mutex::new(VecDeque::new())),
        transferred: Arc::new(AtomicU64::new(0)),
        last_progress_at: std::sync::Mutex::new(None),
        completed_chunks: Arc::new(AtomicU64::new(0)),
        progress: Arc::new(move |value| captured.lock().unwrap().push(value.transferred)),
        cancel,
        chunk_size: 100,
        total_size: 1000,
        chunk_count: 10,
        resumed_from: 0,
        parallelism: 4,
        progress_interval: DEFAULT_PROGRESS_INTERVAL,
        sidecar_state: Arc::new(Mutex::new(SidecarState {
            sidecar: TransferSidecar {
                version: 1,
                direction: TransferDirection::Upload,
                final_path: String::new(),
                part_path: String::new(),
                chunk_size: 100,
                total_size: 1000,
                source_size: 1000,
                source_mtime: None,
                completed: String::new(),
                updated_at_ms: 0,
            },
            bits: vec![],
            unsaved_chunks: 0,
            unsaved_bytes: 0,
            persisted_chunks: 0,
            last_checkpoint: Instant::now(),
        })),
        local_source: PathBuf::new(),
        local_part: PathBuf::new(),
        remote_part: String::new(),
        remote_source: String::new(),
        direction: TransferDirection::Upload,
        read_step: 100,
        write_step: 100,
        checkpointing: false,
        pipeline_window: 1,
        checkpoint_tx: None,
        rate_limiter: None,
    };
    assert_eq!(DEFAULT_PROGRESS_INTERVAL, Duration::from_millis(100));
    report_progress(&shared, 1, false);
    // A future timestamp makes the suppressed case independent of scheduler delays.
    *shared.last_progress_at.lock().unwrap() = Some(Instant::now() + Duration::from_secs(10));
    report_progress(&shared, 2, false);
    *shared.last_progress_at.lock().unwrap() = Some(Instant::now() - Duration::from_millis(101));
    report_progress(&shared, 3, false);
    report_progress(&shared, 1000, true);
    assert_eq!(*reports.lock().unwrap(), vec![1, 3, 1000]);
}
