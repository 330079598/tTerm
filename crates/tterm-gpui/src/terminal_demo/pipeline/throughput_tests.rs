//! Release measurement of the 30-session background throughput.
//!
//! This fixture is deliberately separate from the foreground frame budget. It
//! starts 30 fresh parsers, releases every producer after the timer starts,
//! and requires all 30 MiB to be applied and acknowledged within a fixed
//! 500 ms deadline. It never reads the frame fixture's samples, and the frame
//! fixture never reads these counters.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::tests::state;
use super::{Pipeline, QueueMetrics, RefreshSignal, start};

const SESSIONS: usize = 30;
const SESSION_BYTES: usize = 1024 * 1024;
const TOTAL_BYTES: usize = SESSIONS * SESSION_BYTES;
const BLOCK_BYTES: usize = 16 * 1024;
const LINE_BYTES: usize = 64;
const LINES_PER_BLOCK: usize = BLOCK_BYTES / LINE_BYTES;
const BLOCKS_PER_SESSION: usize = SESSION_BYTES / BLOCK_BYTES;
const DEADLINE: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_micros(200);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(10);

struct Session {
    pipeline: Pipeline,
    refresh: Arc<RefreshSignal>,
    metrics: Arc<QueueMetrics>,
    acknowledged: Arc<AtomicU64>,
}

/// Builds one session's 1 MiB as printable lines ending in newlines.
fn payload(session: usize) -> Vec<Vec<u8>> {
    (0..BLOCKS_PER_SESSION)
        .map(|block| {
            let mut bytes = Vec::with_capacity(BLOCK_BYTES);
            for line in 0..LINES_PER_BLOCK {
                let number = block * LINES_PER_BLOCK + line;
                let prefix = format!("throughput-{session:02}-{number:05}-");
                let fill = LINE_BYTES
                    .checked_sub(prefix.len() + 1)
                    .expect("a throughput line prefix fits");
                bytes.extend_from_slice(prefix.as_bytes());
                bytes.extend(std::iter::repeat_n(b'x', fill));
                bytes.push(b'\n');
            }
            assert_eq!(bytes.len(), BLOCK_BYTES);
            bytes
        })
        .collect()
}

fn start_session() -> Session {
    let (_model, state) = state(false, 32);
    let refresh = Arc::clone(&state.refresh);
    let acknowledged = Arc::new(AtomicU64::new(0));
    let acknowledged_for_pipeline = Arc::clone(&acknowledged);
    let pipeline = start(
        state,
        Arc::new(|_| {}),
        Arc::new(move |bytes| {
            acknowledged_for_pipeline
                .fetch_add(u64::try_from(bytes).unwrap_or(u64::MAX), Ordering::Relaxed);
        }),
        None,
    )
    .expect("start the throughput parser");
    let metrics = pipeline.queue.metrics_handle();
    Session {
        pipeline,
        refresh,
        metrics,
        acknowledged,
    }
}

fn join_settled<T>(handle: JoinHandle<T>, deadline: Instant, what: &str) -> T {
    while !handle.is_finished() {
        assert!(
            Instant::now() < deadline,
            "{what} did not stop before the cleanup deadline"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    handle
        .join()
        .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
}

#[test]
#[ignore = "measures 30-session background throughput; run it through the release gate"]
fn thirty_sessions_apply_and_acknowledge_one_mebibyte_each_within_the_deadline() {
    let sessions: Vec<Session> = (0..SESSIONS).map(|_| start_session()).collect();
    let barrier = Arc::new(Barrier::new(SESSIONS + 1));
    let mut producers = Vec::with_capacity(SESSIONS);
    for (index, session) in sessions.iter().enumerate() {
        let queue = session.pipeline.queue.clone();
        let blocks = payload(index);
        let release = Arc::clone(&barrier);
        producers.push(std::thread::spawn(move || {
            release.wait();
            for block in blocks {
                queue.send(block).expect("send a throughput block");
                std::thread::sleep(Duration::from_millis(2));
            }
        }));
    }

    // No producer can send before this instant: every one waits on the
    // barrier, and the timer starts before the barrier is released.
    let started = Instant::now();
    barrier.wait();
    let elapsed = loop {
        let elapsed = started.elapsed();
        let complete = sessions.iter().all(|session| {
            session.metrics.applied_bytes() == SESSION_BYTES as u64
                && session.acknowledged.load(Ordering::Acquire) == SESSION_BYTES as u64
                && session.metrics.acknowledged_bytes() == SESSION_BYTES as u64
        });
        if complete || elapsed >= DEADLINE {
            break elapsed;
        }
        std::thread::sleep(POLL_INTERVAL.min(DEADLINE.saturating_sub(elapsed)));
    };

    // Capture the measurement before cleanup. Anything a producer or parser
    // does after the deadline is cleanup, not throughput.
    let measured: Vec<(u64, u64, u64)> = sessions
        .iter()
        .map(|session| {
            (
                session.metrics.applied_bytes(),
                session.acknowledged.load(Ordering::Acquire),
                session.metrics.acknowledged_bytes(),
            )
        })
        .collect();
    let applied_total: u64 = measured.iter().map(|(applied, _, _)| applied).sum();
    let acknowledged_total: u64 = measured
        .iter()
        .map(|(_, acknowledged, _)| acknowledged)
        .sum();
    let parsed_total: u64 = measured.iter().map(|(_, _, parsed)| parsed).sum();
    let session_count = sessions.len();
    let refreshes: Vec<u64> = sessions
        .iter()
        .map(|session| session.refresh.sends())
        .collect();
    let completed = measured.iter().all(|&(applied, acknowledged, parsed)| {
        applied == SESSION_BYTES as u64
            && acknowledged == SESSION_BYTES as u64
            && parsed == SESSION_BYTES as u64
    });
    let rate_mib_s = TOTAL_BYTES as f64 / (1024.0 * 1024.0) / elapsed.as_secs_f64();
    eprintln!(
        "P2-02 background throughput: sessions={session_count} bytes={TOTAL_BYTES} \
         applied={applied_total} acknowledged={acknowledged_total} parsed={parsed_total} \
         wall_us={} rate_mib_s={rate_mib_s:.2} deadline_us={} completed={completed}",
        elapsed.as_micros(),
        DEADLINE.as_micros(),
    );

    let cleanup_deadline = Instant::now() + CLEANUP_TIMEOUT;
    for producer in producers {
        join_settled(producer, cleanup_deadline, "throughput producer");
    }
    for session in sessions {
        drop(session.pipeline.queue);
        join_settled(
            session.pipeline.parser,
            cleanup_deadline,
            "throughput parser",
        );
    }

    assert_eq!(session_count, SESSIONS, "a session is missing");
    assert!(
        completed,
        "only {applied_total} of {TOTAL_BYTES} bytes were applied and acknowledged before {DEADLINE:?}"
    );
    assert_eq!(applied_total, TOTAL_BYTES as u64, "applied bytes");
    assert_eq!(acknowledged_total, TOTAL_BYTES as u64, "acknowledged bytes");
    assert_eq!(parsed_total, TOTAL_BYTES as u64, "parsed bytes");
    for (index, (applied, acknowledged, parsed)) in measured.iter().enumerate() {
        assert_eq!(
            *applied, SESSION_BYTES as u64,
            "session {index} applied a different byte count"
        );
        assert_eq!(
            *acknowledged, SESSION_BYTES as u64,
            "session {index} acknowledged a different byte count"
        );
        assert_eq!(
            *parsed, SESSION_BYTES as u64,
            "session {index} parsed a different byte count"
        );
        assert_eq!(
            refreshes[index], 0,
            "hidden session {index} requested a foreground frame"
        );
    }
    assert!(
        elapsed <= DEADLINE,
        "the 30 MiB transfer took {elapsed:?}, past the {DEADLINE:?} deadline"
    );
}
