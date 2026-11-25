//! Behaviour tests for the GPUI parser, bounded queue and refresh signal.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Condvar, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext as _, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions, point, px, size,
};
use tterm_core::terminal::io_batcher::OutputFlow;
use tterm_core::terminal::output::TerminalOutput;
use tterm_core::terminal::ssh_query_handler::ordered::OrderedQueries;
use tterm_terminal::grid::{CellSize, GridPoint, GridSize, WindowSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::mouse_guard::MouseGuard;
use super::{
    BATCH_LIMIT, MAX_QUEUED_BYTES, Pipeline, QUEUED_CHUNKS, QueueMetrics, RefreshSignal,
    SessionState, start,
};

const GRID: GridSize = GridSize {
    columns: 80,
    screen_lines: 24,
};
const CELL: CellSize = CellSize {
    width: 8,
    height: 16,
};
const WAIT: Duration = Duration::from_secs(15);

pub(super) fn state(visible: bool, scrollback: usize) -> (Arc<Terminal>, SessionState) {
    let terminal = Arc::new(Terminal::new(
        Config {
            scrollback,
            ..Config::default()
        },
        GRID,
        CELL,
    ));
    let state = SessionState {
        terminal: Arc::clone(&terminal),
        size: Arc::new(Mutex::new(WindowSize {
            grid: GRID,
            cell: CELL,
        })),
        title: Arc::new(Mutex::new(None)),
        clipboard: Arc::new(crate::terminal_demo::clipboard::store::ClipboardWrites::disabled()),
        session_nonce: 1,
        alive: Arc::new(AtomicBool::new(true)),
        visible: Arc::new(AtomicBool::new(visible)),
        refresh: Arc::new(RefreshSignal::new()),
        reports: Arc::new(crate::terminal_demo::session::reports::Reports::new()),
    };
    (terminal, state)
}

fn start_pipeline(
    state: SessionState,
    acknowledge: impl Fn(usize) + Send + Sync + 'static,
) -> Pipeline {
    start(state, Arc::new(|_| {}), Arc::new(acknowledge), None, None).expect("start the parser")
}

fn start_ssh_pipeline(state: SessionState) -> (Pipeline, Arc<Mutex<Vec<u8>>>) {
    let replies = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&replies);
    let pipeline = start(
        state,
        Arc::new(move |bytes| {
            captured
                .lock()
                .expect("captured SSH replies")
                .extend_from_slice(&bytes);
        }),
        Arc::new(|_| {}),
        None,
        Some(OrderedQueries::default()),
    )
    .expect("start the SSH parser");
    (pipeline, replies)
}

fn text(model: &Terminal) -> String {
    model
        .snapshot()
        .rows()
        .iter()
        .flat_map(|row| row.cells())
        .map(|cell| cell.ch)
        .collect()
}

fn wait_until(predicate: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + WAIT;
    loop {
        if predicate() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn output_into(pipeline: &Pipeline, flow: Arc<OutputFlow>) -> TerminalOutput {
    let queue = pipeline.queue.clone();
    TerminalOutput::new(Arc::new(move |bytes| queue.send(bytes)), flow)
}

#[test]
fn the_parser_stage_byte_ceiling_is_unchanged() {
    assert_eq!(MAX_QUEUED_BYTES, 589_824);
}

#[test]
fn sustained_output_keeps_the_queue_bounded_and_applies_every_byte() {
    let (model, state) = state(true, 64);
    let flow = Arc::new(OutputFlow::default());
    let acknowledge_flow = Arc::clone(&flow);
    let pipeline = start_pipeline(state, move |bytes| acknowledge_flow.ack(bytes));
    let metrics = pipeline.queue.metrics_handle();
    let output = output_into(&pipeline, flow);

    const CHUNKS: usize = 128;
    let mut sent = 0_usize;
    for index in 0..CHUNKS {
        let line = format!("line-{index:06}-{}\r\n", "x".repeat(48));
        sent += line.len();
        output.send(line.into_bytes()).expect("queue output");
    }
    let marker = b"\r\nEND-OF-SUSTAINED-OUTPUT\r\n";
    sent += marker.len();
    output.send(marker.to_vec()).expect("queue marker");
    drop(output);
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");

    assert_eq!(metrics.sent_bytes(), sent as u64);
    assert_eq!(metrics.acknowledged_bytes(), sent as u64);
    assert!(
        metrics.max_queued_bytes() <= MAX_QUEUED_BYTES,
        "queued high-water mark {} exceeds {}: {:?}",
        metrics.max_queued_bytes(),
        MAX_QUEUED_BYTES,
        metrics.max_queued_bytes()
    );
    let frame = model.snapshot();
    assert!(
        frame.history_size() <= 64,
        "the model keeps its configured bounded scrollback"
    );
    assert_eq!(frame.line_count(), GRID.screen_lines);
    assert!(text(&model).contains("END-OF-SUSTAINED-OUTPUT"));
}

/// A gate the acknowledgement callback can wait on in the flow-control test.
#[derive(Default)]
struct AckGate {
    open: Mutex<bool>,
    changed: Condvar,
    waits: AtomicUsize,
}

impl AckGate {
    fn wait_until_open(&self) {
        self.waits.fetch_add(1, Ordering::AcqRel);
        let mut open = self.open.lock().expect("ack gate");
        while !*open {
            open = self.changed.wait(open).expect("wait for ack gate");
        }
    }

    fn open(&self) {
        *self.open.lock().expect("ack gate") = true;
        self.changed.notify_all();
    }

    fn waits(&self) -> usize {
        self.waits.load(Ordering::Acquire)
    }
}

#[test]
fn a_blocked_acknowledgement_stops_the_producer_within_the_memory_bound() {
    let (model, state) = state(true, 64);
    let flow = Arc::new(OutputFlow::default());
    let gate = Arc::new(AckGate::default());
    let acknowledge_flow = Arc::clone(&flow);
    let acknowledge_gate = Arc::clone(&gate);
    let pipeline = start_pipeline(state, move |bytes| {
        acknowledge_gate.wait_until_open();
        acknowledge_flow.ack(bytes);
    });
    let metrics = pipeline.queue.metrics_handle();
    let output = output_into(&pipeline, Arc::clone(&flow));

    const TOTAL: usize = 8 * 1024 * 1024;
    let progress = Arc::new(AtomicUsize::new(0));
    let producer_progress = Arc::clone(&progress);
    let producer = std::thread::spawn(move || {
        let mut sent = 0;
        while sent < TOTAL {
            let length = BATCH_LIMIT.min(TOTAL - sent);
            output
                .send(vec![b'z'; length])
                .expect("send sustained output");
            sent += length;
            producer_progress.store(sent, Ordering::Release);
        }
        output.send(b"\r\nEND-OF-BLOCKED-ACK\r\n".to_vec()).unwrap();
    });

    assert!(
        wait_until(|| progress.load(Ordering::Acquire) > 0),
        "the producer starts before the parser blocks"
    );
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !producer.is_finished(),
        "the output flow stops the producer while acknowledgements wait"
    );
    assert!(
        metrics.queued_bytes() <= MAX_QUEUED_BYTES,
        "queued bytes stay bounded while the parser waits for an ack"
    );
    assert!(
        metrics.max_queued_bytes() <= MAX_QUEUED_BYTES,
        "the high-water mark stays bounded: {} > {}",
        metrics.max_queued_bytes(),
        MAX_QUEUED_BYTES
    );
    eprintln!(
        "P2-02 blocked ACK: producer_progress={} bytes, queued_high_water={} bytes, limit={} bytes",
        progress.load(Ordering::Acquire),
        metrics.max_queued_bytes(),
        MAX_QUEUED_BYTES
    );

    gate.open();
    producer.join().expect("producer finishes after the ack");
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
    assert!(text(&model).contains("END-OF-BLOCKED-ACK"));
    assert_eq!(metrics.acknowledged_bytes(), metrics.sent_bytes());
    eprintln!(
        "P2-02 blocked ACK drained: sent={} bytes, acknowledged={} bytes",
        metrics.sent_bytes(),
        metrics.acknowledged_bytes()
    );
}

#[test]
fn concurrent_producers_share_one_queue_budget() {
    const PRODUCERS: usize = 16;

    let (_model, state) = state(true, 64);
    let flow = Arc::new(OutputFlow::default());
    let gate = Arc::new(AckGate::default());
    let acknowledge_flow = Arc::clone(&flow);
    let acknowledge_gate = Arc::clone(&gate);
    let pipeline = start_pipeline(state, move |bytes| {
        acknowledge_gate.wait_until_open();
        acknowledge_flow.ack(bytes);
    });
    let metrics = pipeline.queue.metrics_handle();
    let barrier = Arc::new(Barrier::new(PRODUCERS));
    let started = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicUsize::new(0));
    let mut producers = Vec::with_capacity(PRODUCERS);

    for index in 0..PRODUCERS {
        let queue = pipeline.queue.clone();
        let barrier = Arc::clone(&barrier);
        let started = Arc::clone(&started);
        let finished = Arc::clone(&finished);
        producers.push(std::thread::spawn(move || {
            barrier.wait();
            started.fetch_add(1, Ordering::Release);
            let result = queue.send(vec![b'a' + index as u8; BATCH_LIMIT]);
            if result.is_ok() {
                finished.fetch_add(1, Ordering::Release);
            }
            result
        }));
    }

    assert!(wait_until(|| started.load(Ordering::Acquire) == PRODUCERS));
    assert!(
        wait_until(|| gate.waits() > 0),
        "the parser reaches the blocked acknowledgement"
    );
    assert!(
        wait_until(|| finished.load(Ordering::Acquire) > QUEUED_CHUNKS),
        "the parser holds one batch while the queue fills to capacity"
    );

    gate.open();
    let results: Vec<_> = producers
        .into_iter()
        .map(|producer| producer.join())
        .collect();
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
    let max_active_sends = metrics.max_active_sends();
    let observed_high_water = metrics.max_queued_bytes();

    assert!(
        results.iter().all(Result::is_ok),
        "a concurrent producer panicked"
    );
    assert_eq!(
        max_active_sends, 1,
        "more than one producer entered a reserved send"
    );
    assert!(
        observed_high_water <= MAX_QUEUED_BYTES,
        "concurrent producers exceeded the shared budget: {observed_high_water} > {MAX_QUEUED_BYTES}"
    );
    assert_eq!(
        metrics.acknowledged_bytes(),
        (PRODUCERS * BATCH_LIMIT) as u64
    );
}

#[test]
fn synthetic_output_is_parsed_without_acknowledging_flow_bytes() {
    let (model, state) = state(true, 64);
    let acknowledged = Arc::new(AtomicUsize::new(0));
    let acknowledge_counter = Arc::clone(&acknowledged);
    let pipeline = start_pipeline(state, move |bytes| {
        acknowledge_counter.fetch_add(bytes, Ordering::Relaxed);
    });
    let metrics = pipeline.queue.metrics_handle();
    let output = b"REAL-OUTPUT\r\n";
    let synthetic = b"SYNTHETIC-EXIT\r\n";

    pipeline.queue.send(output.to_vec()).expect("queue output");
    pipeline
        .queue
        .send_synthetic(synthetic.to_vec())
        .expect("queue synthetic output");
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");

    let screen = text(&model);
    assert!(screen.contains("REAL-OUTPUT"));
    assert!(screen.contains("SYNTHETIC-EXIT"));
    assert_eq!(
        acknowledged.load(Ordering::Acquire),
        output.len(),
        "synthetic output must not consume PTY flow-control credit"
    );
    assert_eq!(
        metrics.acknowledged_bytes(),
        (output.len() + synthetic.len()) as u64,
        "the parser still accounts for every displayed byte"
    );
}

#[test]
fn every_acknowledgement_follows_the_model_write() {
    let (model, state) = state(true, 64);
    let observed = Arc::new(Mutex::new(Vec::new()));
    let ack_model = Arc::clone(&model);
    let ack_observed = Arc::clone(&observed);
    let pipeline = start_pipeline(state, move |bytes| {
        let screen = text(&ack_model);
        assert!(
            screen.contains("ACK-ORDER-MARKER"),
            "an acknowledgement arrived before the bytes reached the model"
        );
        ack_observed.lock().expect("ack observations").push(bytes);
    });

    pipeline
        .queue
        .send(b"ACK-ORDER-MARKER\r\n".to_vec())
        .expect("send marker");
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
    assert_eq!(
        observed.lock().expect("ack observations").as_slice(),
        &[b"ACK-ORDER-MARKER\r\n".len()]
    );
}

#[test]
fn acknowledgement_follows_query_replies_and_other_parser_intents() {
    let (_, state) = state(true, 64);
    let replies = Arc::new(Mutex::new(Vec::new()));
    let write_replies = Arc::clone(&replies);
    let ack_replies = Arc::clone(&replies);
    let pipeline = start(
        state,
        Arc::new(move |bytes| {
            write_replies.lock().expect("query replies").push(bytes);
        }),
        Arc::new(move |_| {
            assert_eq!(
                ack_replies.lock().expect("query replies").as_slice(),
                &[b"\x1b[4;384;640t".to_vec()],
                "the parser acknowledged before answering the terminal query"
            );
        }),
        None,
        None,
    )
    .expect("start the parser");

    pipeline
        .queue
        .send(b"\x1b[14t".to_vec())
        .expect("send a window-size query");
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
}

#[test]
fn a_forwarded_cursor_position_report_is_answered_from_the_model_grid() {
    let (model, state) = state(true, 64);
    // The SSH reader forwards a plain cursor position query instead of
    // answering it, so the reply must carry the model's cursor, not home.
    model.write(b"abc");
    assert!(model.drain_intents().is_empty());
    let replies = Arc::new(Mutex::new(Vec::new()));
    let write_replies = Arc::clone(&replies);
    let pipeline = start(
        state,
        Arc::new(move |bytes| {
            write_replies.lock().expect("query replies").push(bytes);
        }),
        Arc::new(|_| {}),
        None,
        None,
    )
    .expect("start the parser");

    pipeline
        .queue
        .send(b"\x1b[6n".to_vec())
        .expect("send a cursor position query");
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");

    assert_eq!(
        replies.lock().expect("query replies").as_slice(),
        &[b"\x1b[1;4R".to_vec()],
        "the model answers with the cursor one column past the three characters"
    );
}

#[test]
fn a_forwarded_status_report_is_answered_by_the_model() {
    let (_, state) = state(true, 64);
    let replies = Arc::new(Mutex::new(Vec::new()));
    let write_replies = Arc::clone(&replies);
    let pipeline = start(
        state,
        Arc::new(move |bytes| {
            write_replies.lock().expect("query replies").push(bytes);
        }),
        Arc::new(|_| {}),
        None,
        None,
    )
    .expect("start the parser");

    pipeline
        .queue
        .send(b"\x1b[5n".to_vec())
        .expect("send a status report query");
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");

    assert_eq!(
        replies.lock().expect("query replies").as_slice(),
        &[b"\x1b[0n".to_vec()],
        "the model still reports ready, now through its ordered reply path"
    );
}

#[test]
fn shutdown_acknowledges_rejected_bytes_so_the_reader_cannot_wedge() {
    let (model, state) = state(true, 64);
    state.alive.store(false, Ordering::Release);
    let acknowledged = Arc::new(AtomicU64::new(0));
    let ack_counter = Arc::clone(&acknowledged);
    let pipeline = start_pipeline(state, move |bytes| {
        ack_counter.fetch_add(bytes as u64, Ordering::Relaxed);
    });

    let payload = b"output after the session closed\r\n".repeat(100);
    pipeline.queue.send(payload.clone()).expect("queue output");
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");

    assert_eq!(acknowledged.load(Ordering::Acquire), payload.len() as u64);
    assert!(
        !text(&model).contains("output after the session closed"),
        "a closed generation never writes into its model"
    );
}

#[test]
fn hidden_sessions_parse_but_do_not_raise_refresh_notifications() {
    let (model, state) = state(false, 64);
    let refresh = Arc::clone(&state.refresh);
    let visible = Arc::clone(&state.visible);
    let pipeline = start_pipeline(state, |_| {});

    pipeline
        .queue
        .send(b"HIDDEN-SESSION-OUTPUT\r\n".to_vec())
        .expect("send hidden output");
    assert!(wait_until(|| text(&model).contains("HIDDEN-SESSION-OUTPUT")));
    assert_eq!(
        refresh.sends(),
        0,
        "a hidden session does not ask the window for a frame"
    );

    visible.store(true, Ordering::Release);
    pipeline
        .queue
        .send(b"VISIBLE-SESSION-OUTPUT\r\n".to_vec())
        .expect("send visible output");
    assert!(wait_until(|| refresh.sends() > 0));
    assert!(text(&model).contains("VISIBLE-SESSION-OUTPUT"));

    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
}

#[test]
fn refresh_notifications_are_coalesced_until_the_view_acknowledges() {
    let refresh = RefreshSignal::new();
    for _ in 0..10_000 {
        refresh.notify();
    }
    assert_eq!(refresh.sends(), 1, "a flood leaves one pending wake");

    refresh.acknowledge();
    refresh.notify();
    assert_eq!(refresh.sends(), 2, "the next frame is requested again");
}

#[test]
fn a_reattached_view_can_receive_refreshes_after_output_while_it_was_gone() {
    let refresh = RefreshSignal::new();
    refresh.notify();
    assert_eq!(refresh.sends(), 1);

    let _receiver = refresh.subscribe();
    refresh.notify();
    assert_eq!(
        refresh.sends(),
        2,
        "subscribing releases the pending slot without waiting for a frame"
    );
}

#[test]
fn thirty_hidden_sessions_parse_without_waking_the_foreground() {
    let mut sessions: Vec<(Arc<Terminal>, Arc<RefreshSignal>, Pipeline)> = Vec::new();
    for index in 0..30 {
        let (model, state) = state(false, 32);
        let refresh = Arc::clone(&state.refresh);
        let pipeline = start_pipeline(state, |_| {});
        let marker = format!("BACKGROUND-SESSION-{index:02}-END\r\n");
        for line in 0..8 {
            let chunk = format!(
                "background-{index:02}-line-{line:02}-{}\r\n",
                "b".repeat(48)
            );
            pipeline
                .queue
                .send(chunk.into_bytes())
                .expect("queue background output");
        }
        pipeline
            .queue
            .send(marker.into_bytes())
            .expect("queue background marker");
        sessions.push((model, refresh, pipeline));
    }

    for (index, (model, refresh, _pipeline)) in sessions.iter().enumerate() {
        let marker = format!("BACKGROUND-SESSION-{index:02}-END");
        assert!(wait_until(|| text(model).contains(&marker)));
        assert_eq!(
            refresh.sends(),
            0,
            "a hidden background session never requests a foreground frame"
        );
    }
    for (_, _, pipeline) in sessions {
        drop(pipeline.queue);
        pipeline.parser.join().expect("parser exits");
    }
}

// This test times wall-clock frames while 30 parsers saturate the machine, so
// it needs the machine to itself. In the default parallel suite the remaining
// tests compete for the cores those parsers occupy and the result describes
// the scheduler rather than 30 background sessions (W0-05, decision 019).
// The release gate runs it alone on an otherwise idle machine; see
// scripts/perf/gpui-release-frame-budget.mjs.
#[ignore = "measures wall-clock frames under 30 background parsers; run it through the release gate"]
#[gpui_kit::test]
fn thirty_background_parsers_keep_the_foreground_within_its_frame_budget(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (foreground, _) = state(true, 1_000);
    let bounds = Bounds {
        origin: point(px(0.0), px(0.0)),
        size: size(px(1200.0), px(800.0)),
    };
    let foreground_for_view = Arc::clone(&foreground);
    let (handle, view): (gpui_kit::AnyWindowHandle, Entity<TerminalView>) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            cx,
            |_, cx| {
                cx.new(|cx| {
                    TerminalView::new(cx, foreground_for_view, TerminalViewConfig::default())
                })
            },
        )
        .expect("open the foreground terminal")
    });

    // Warm the text system and prove the measurement path produces real frames.
    let _ = measure_frames(cx, handle, &foreground, 12);
    let baseline = measure_frames(cx, handle, &foreground, 120);

    let stop = Arc::new(AtomicBool::new(false));
    let mut sessions = Vec::new();
    let mut producers = Vec::new();
    for index in 0..30 {
        let (_, background_state) = state(false, 32);
        let refresh = Arc::clone(&background_state.refresh);
        let pipeline = start_pipeline(background_state, |_| {});
        let metrics = pipeline.queue.metrics_handle();
        let queue = pipeline.queue.clone();
        let producer_stop = Arc::clone(&stop);
        producers.push(std::thread::spawn(move || {
            let payload =
                format!("background-{index:02}-{}\r\n", "b".repeat(BATCH_LIMIT / 4)).into_bytes();
            while !producer_stop.load(Ordering::Acquire) {
                queue
                    .send(payload.clone())
                    .expect("queue background output");
                // P0-01's continuous workloads move megabytes per second.
                // Pacing these producers keeps the measurement about the
                // application workload instead of an artificial all-core
                // spin loop.
                std::thread::sleep(Duration::from_millis(2));
            }
        }));
        sessions.push((pipeline, refresh, metrics));
    }
    assert!(
        wait_until(|| sessions.iter().all(|(_, _, metrics)| metrics.batches() > 0)),
        "all background parsers become active"
    );
    let warmup_start = acknowledged_snapshot(&sessions);
    std::thread::sleep(Duration::from_millis(100));

    // The warm-up window proves all 30 parsers advanced before the frame
    // window. The frame window's own snapshot proves the sampled frames had
    // real background work rather than an idle machine.
    let warmup_end = acknowledged_snapshot(&sessions);
    let warmup_advanced = advanced_sessions(&warmup_start, &warmup_end);
    let warmup_acked: u64 = warmup_end.iter().sum();
    assert_eq!(
        warmup_advanced,
        sessions.len(),
        "the 100 ms warm-up moved only {warmup_advanced} of {} background sessions",
        sessions.len()
    );
    let frame_window_started = Instant::now();
    foreground.reset_snapshot_timing();
    let loaded = measure_frames(cx, handle, &foreground, 120);
    let frame_window = frame_window_started.elapsed();
    let frame_end = acknowledged_snapshot(&sessions);
    let frame_window_advanced = advanced_sessions(&warmup_end, &frame_end);
    let frame_window_acked: u64 = frame_end
        .iter()
        .zip(&warmup_end)
        .map(|(end, start)| end.saturating_sub(*start))
        .sum();
    stop.store(true, Ordering::Release);
    for producer in producers {
        producer.join().expect("background producer stops");
    }
    let hidden_refreshes: u64 = sessions.iter().map(|(_, refresh, _)| refresh.sends()).sum();
    assert_eq!(hidden_refreshes, 0);
    let acknowledged: u64 = sessions
        .iter()
        .map(|(_, _, metrics)| metrics.acknowledged_bytes())
        .sum();
    for (pipeline, _, _) in sessions {
        drop(pipeline.queue);
        pipeline.parser.join().expect("background parser stops");
    }

    let baseline_p50 = percentile(&baseline, 50);
    let baseline_p95 = percentile(&baseline, 95);
    let loaded_p50 = percentile(&loaded, 50);
    let loaded_p95 = percentile(&loaded, 95);
    let stats = view.read_with(cx, |view, _| view.stats());
    let timing = foreground.snapshot_timing();
    let frame_window_mib_s =
        frame_window_acked as f64 / (1024.0 * 1024.0) / frame_window.as_secs_f64();
    eprintln!(
        "P2-02 foreground frames: baseline p50={baseline_p50:?} p95={baseline_p95:?}; \
         30-background p50={loaded_p50:?} p95={loaded_p95:?}; snapshots={} frames={}; \
         baseline_frames={} loaded_frames={} loaded_p95_us={}; \
         bytes={acknowledged}; warmup_acked={warmup_acked}; \
         warmup_advanced={warmup_advanced}; frame_window={frame_window:?}; \
         frame_window_acked={frame_window_acked} ({frame_window_mib_s:.2} MiB/s); \
         frame_window_advanced={frame_window_advanced}; snapshot_wait={:?} total, \
         snapshot_hold={:?} total / {:?} max over {} snapshots",
        stats.snapshots,
        stats.frames,
        baseline.len(),
        loaded.len(),
        loaded_p95.as_micros(),
        timing.waited,
        timing.held,
        timing.held_max,
        timing.snapshots,
    );
    // The frame budget no longer depends on a cumulative byte floor: a short,
    // fast release window cannot be compared with a fixed 30 MiB total. The
    // throughput fixture measures that total separately. Here a sampled window
    // with no background progress must still fail rather than look idle-green.
    assert!(
        frame_window_acked > 0,
        "the frame window processed no bytes across 30 background sessions"
    );
    assert!(
        frame_window_advanced > 0,
        "the frame window had no background session advance"
    );
    assert!(
        loaded_p95 <= Duration::from_millis(16) + Duration::from_micros(667),
        "30 background parsers pushed the foreground p95 frame time to {loaded_p95:?}"
    );
}

fn acknowledged_snapshot(
    sessions: &[(Pipeline, Arc<RefreshSignal>, Arc<QueueMetrics>)],
) -> Vec<u64> {
    sessions
        .iter()
        .map(|(_, _, metrics)| metrics.acknowledged_bytes())
        .collect()
}

fn advanced_sessions(before: &[u64], after: &[u64]) -> usize {
    before
        .iter()
        .zip(after)
        .filter(|(before, after)| after > before)
        .count()
}

fn measure_frames(
    cx: &mut TestAppContext,
    handle: gpui_kit::AnyWindowHandle,
    model: &Terminal,
    frames: usize,
) -> Vec<Duration> {
    let mut samples = Vec::with_capacity(frames);
    for index in 0..frames {
        model.write(format!("foreground-{index:04}\r\n").as_bytes());
        let started = Instant::now();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .expect("render the foreground terminal");
        samples.push(started.elapsed());
    }
    samples
}

fn percentile(samples: &[Duration], percentile: usize) -> Duration {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[(sorted.len() * percentile / 100).min(sorted.len() - 1)]
}

#[test]
fn a_visible_session_still_gets_one_refresh_per_acknowledged_frame() {
    let (model, state) = state(true, 64);
    let refresh = Arc::clone(&state.refresh);
    let pipeline = start_pipeline(state, |_| {});
    let metrics: Arc<QueueMetrics> = pipeline.queue.metrics_handle();

    let mut previous_sends = 0;
    for index in 0..64 {
        pipeline
            .queue
            .send(format!("visible-{index:03}\r\n").into_bytes())
            .expect("queue visible output");
        assert!(wait_until(
            || text(&model).contains(&format!("visible-{index:03}"))
        ));
        assert!(
            wait_until(|| refresh.sends() > previous_sends),
            "each parsed batch requests one frame"
        );
        previous_sends = refresh.sends();
        refresh.acknowledge();
    }
    assert_eq!(previous_sends, 64);
    assert_eq!(metrics.batches(), 64);

    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
}

#[test]
fn a_prompt_mark_closes_mouse_reporting_inside_the_parser() {
    let (model, state) = state(true, 64);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let guard = Arc::new(MouseGuard::new(
        state.clone(),
        runtime.handle().clone(),
        None,
    ));
    let pipeline = start(state, Arc::new(|_| {}), Arc::new(|_| {}), Some(guard), None)
        .expect("start the parser");
    pipeline
        .queue
        .send(b"\x1b[?1002h\x1b[?1006h$ \x1b]133;A\x07".to_vec())
        .expect("queue output");
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
    assert!(!model.is_mouse_reporting());
}

fn wait_for_clipboard(
    writes: &crate::terminal_demo::clipboard::store::ClipboardWrites,
) -> Option<String> {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(text) = writes.take() {
            return Some(text);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn osc52_writes_cross_from_a_hidden_parser_to_one_bounded_slot() {
    let (_, mut state) = state(false, 64);
    let clipboard = Arc::new(crate::terminal_demo::clipboard::store::ClipboardWrites::new(true));
    state.clipboard = Arc::clone(&clipboard);
    let pipeline = start_pipeline(state, |_| {});
    let output = output_into(&pipeline, Arc::new(OutputFlow::default()));

    // BEL terminator, ordinary clipboard target.
    output
        .send(b"\x1b]52;c;aGVsbG8gd29ybGQ=\x07".to_vec())
        .expect("queue BEL write");
    assert_eq!(
        wait_for_clipboard(&clipboard).as_deref(),
        Some("hello world")
    );

    // ST terminator, selection target still writes the ordinary clipboard.
    output
        .send(b"\x1b]52;p;5L2g5aW977yMdmltCg==\x1b\\".to_vec())
        .expect("queue ST write");
    assert_eq!(
        wait_for_clipboard(&clipboard).as_deref(),
        Some("你好，vim\n")
    );

    // Query, empty payload, invalid target and invalid base64 are not stores;
    // the next valid write is still delivered.
    for ignored in [
        b"\x1b]52;c;?\x07".as_slice(),
        b"\x1b]52;c;\x07".as_slice(),
        b"\x1b]52;x;aGVsbG8=\x07".as_slice(),
        b"\x1b]52;c;not base64!\x07".as_slice(),
    ] {
        output.send(ignored.to_vec()).expect("queue ignored write");
    }
    output
        .send(b"\x1b]52;s;bGFzdA==\x07".to_vec())
        .expect("queue final write");
    assert_eq!(wait_for_clipboard(&clipboard).as_deref(), Some("last"));

    drop(output);
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
}

#[test]
fn a_disabled_clipboard_setting_rejects_osc52_at_the_slot() {
    let (_, mut state) = state(true, 64);
    let clipboard = Arc::new(crate::terminal_demo::clipboard::store::ClipboardWrites::new(false));
    state.clipboard = Arc::clone(&clipboard);
    let pipeline = start_pipeline(state, |_| {});
    let output = output_into(&pipeline, Arc::new(OutputFlow::default()));
    output
        .send(b"\x1b]52;c;aGVsbG8=\x07\x1b[?25h".to_vec())
        .expect("queue write");
    drop(output);
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
    assert_eq!(clipboard.take(), None);
}

#[test]
fn a_timeout_flush_does_not_replay_osc52_inside_the_block() {
    let (model, mut state) = state(false, 64);
    let clipboard = Arc::new(crate::terminal_demo::clipboard::store::ClipboardWrites::new(true));
    state.clipboard = Arc::clone(&clipboard);
    let pipeline = start_pipeline(state, |_| {});
    let output = output_into(&pipeline, Arc::new(OutputFlow::default()));

    output
        .send(b"\x1b[?2026h\x1b]52;c;aGVsbG8=\x07".to_vec())
        .expect("queue a store inside an unclosed update");
    assert_eq!(
        wait_for_clipboard(&clipboard).as_deref(),
        Some("hello"),
        "the store crosses to the slot before the block is displayed"
    );
    assert!(
        wait_until(|| model.has_synchronized_output()),
        "the update holds the bytes"
    );
    assert!(
        wait_until(|| !model.has_synchronized_output()),
        "the timeout aborts the update"
    );
    assert_eq!(
        clipboard.take(),
        None,
        "aborting the update must not replay its OSC 52 store"
    );

    output.send(b"AFTER".to_vec()).expect("queue later output");
    assert!(wait_until(|| text(&model).contains("AFTER")));
    assert_eq!(
        clipboard.take(),
        None,
        "output after the abort must not replay the store either"
    );

    drop(output);
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
}

const SSH_DA1_REPLY: &[u8] = b"\x1b[?64;1;2;4;6;9;15;21;22c";

fn run_ssh_output(chunks: &[Vec<u8>]) -> (Vec<u8>, Arc<Terminal>) {
    let (model, state) = state(false, 64);
    let (pipeline, replies) = start_ssh_pipeline(state);
    for chunk in chunks {
        pipeline
            .queue
            .send(chunk.clone())
            .expect("queue SSH output");
    }
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
    let replies = replies.lock().expect("captured replies").clone();
    (replies, model)
}

#[test]
fn synchronized_output_keeps_fixed_and_model_replies_in_wire_order() {
    let input = [
        b"\x1b[?2026h".as_slice(),
        b"\x1b[4;7H",
        b"\x1b[6n",
        b"\x1b[c",
        b"\x1b[9;3H",
        b"\x1b[6n",
        b"\x1b[?2026l",
        b"\x1b[2;5H",
        b"\x1b[6n",
    ]
    .concat();
    let expected = [
        b"\x1b[4;7R".as_slice(),
        SSH_DA1_REPLY,
        b"\x1b[9;3R",
        b"\x1b[2;5R",
    ]
    .concat();

    for chunk_size in [input.len(), 1, 3, 7, 11] {
        let chunks = input
            .chunks(chunk_size)
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        let (replies, model) = run_ssh_output(&chunks);
        assert_eq!(
            replies, expected,
            "chunk size {chunk_size}: replies must follow the stream, not the responder"
        );
        assert_eq!(
            model.snapshot().cursor().point.column,
            4,
            "the ordinary query after ESU runs at the final cursor column"
        );
        assert_eq!(
            model.snapshot().cursor().point.line,
            1,
            "the ordinary query after ESU runs at the final cursor line"
        );
    }
}

#[test]
fn synchronized_output_keeps_fixed_replies_ahead_of_later_model_replies() {
    let input = [
        b"\x1b[?2026h".as_slice(),
        b"\x1b[c",
        b"\x1b[6;4H",
        b"\x1b[6n",
        b"\x1b[>c",
        b"\x1b[?2026l",
        b"\x1b[5n",
    ]
    .concat();
    let expected = [
        SSH_DA1_REPLY,
        b"\x1b[6;4R".as_slice(),
        b"\x1b[>0;276;0c".as_slice(),
        b"\x1b[0n".as_slice(),
    ]
    .concat();

    for chunk_size in [input.len(), 1, 2, 5, 13] {
        let chunks = input
            .chunks(chunk_size)
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        let (replies, _) = run_ssh_output(&chunks);
        assert_eq!(
            replies, expected,
            "chunk size {chunk_size}: a fixed reply before a model query must stay before it"
        );
    }
}

#[test]
fn synchronized_output_stays_hidden_until_its_end() {
    let (model, state) = state(true, 64);
    let (pipeline, replies) = start_ssh_pipeline(state);
    let metrics = pipeline.queue.metrics_handle();
    let hidden = b"\x1b[?2026hHELD\x1b[4;7H\x1b[6n\x1b[c";
    pipeline
        .queue
        .send(hidden.to_vec())
        .expect("queue an open synchronized update");
    assert!(
        wait_until(|| metrics.acknowledged_bytes() == hidden.len() as u64),
        "the parser consumes the open block"
    );
    let snapshot = model.snapshot();
    assert!(!text(&model).contains("HELD"));
    assert_eq!(snapshot.cursor().point, GridPoint::new(0, 0));
    assert!(
        replies.lock().expect("captured replies").is_empty(),
        "a model query inside the block must not force a reply before ESU"
    );

    let tail = b"\x1b[?2026l SHOWN";
    pipeline
        .queue
        .send(tail.to_vec())
        .expect("queue the synchronized update's end");
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
    let snapshot = model.snapshot();
    assert!(text(&model).contains("SHOWN"));
    assert_eq!(snapshot.cursor().point, GridPoint::new(3, 12));
    assert_eq!(
        *replies.lock().expect("captured replies"),
        [b"\x1b[4;7R".as_slice(), SSH_DA1_REPLY].concat(),
        "the buffered model reply stays before the deferred fixed reply"
    );
}

#[test]
fn synchronized_output_defers_capability_replies_too() {
    let input = [
        b"\x1b[?2026h".as_slice(),
        b"\x1b[?6n",
        b"\x1b[3;2H\x1b[6n",
        b"\x1bP+q436f\x1b\\",
        b"\x1b[?2026l",
    ]
    .concat();
    let expected = [
        b"\x1b[?1;1R".as_slice(),
        b"\x1b[3;2R".as_slice(),
        b"\x1bP1+r436f3d323536\x1b\\".as_slice(),
    ]
    .concat();

    for chunk_size in [input.len(), 1, 3, 7] {
        let chunks = input
            .chunks(chunk_size)
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        let (replies, _) = run_ssh_output(&chunks);
        assert_eq!(
            replies, expected,
            "chunk size {chunk_size}: a capability answer keeps its stream position"
        );
    }
}

#[test]
fn a_real_private_mode_query_never_consumes_an_ordering_reply() {
    let mode_query = b"\x1b[?57344$p";
    let mode_report = b"\x1b[?57344;0$y";
    let input = [
        b"\x1b[?2026h".as_slice(),
        b"\x1b[4;7H\x1b[6n",
        mode_query,
        b"\x1b[c",
        mode_query,
        b"\x1b[9;3H\x1b[6n",
        mode_query,
        b"\x1b[?6n",
        b"\x1b[?2026l",
        b"\x1b[2;5H\x1b[6n",
    ]
    .concat();
    let expected = [
        b"\x1b[4;7R".as_slice(),
        mode_report,
        SSH_DA1_REPLY,
        mode_report,
        b"\x1b[9;3R",
        mode_report,
        b"\x1b[?1;1R",
        b"\x1b[2;5R",
    ]
    .concat();

    for chunk_size in [input.len(), 1, 2, 3, 7, 11, 16] {
        let chunks = input
            .chunks(chunk_size)
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        let (replies, _) = run_ssh_output(&chunks);
        assert_eq!(
            replies, expected,
            "chunk size {chunk_size}: the program's own 57344 report keeps its place"
        );
    }
}

const FOREGROUND_COLOR_REPLY: &[u8] = b"\x1b]10;rgb:d7d7/dddd/e8e8\x07";
const BACKGROUND_COLOR_REPLY: &[u8] = b"\x1b]11;rgb:1b1b/1d1d/2323\x07";
const CURSOR_COLOR_REPLY: &[u8] = b"\x1b]12;rgb:7979/d8d8/ffff\x07";
const INDEXED_BLACK_REPLY: &[u8] = b"\x1b]4;0;rgb:1616/1b1b/2222\x07";

#[test]
fn color_queries_are_answered_once_by_the_model() {
    let input = [
        b"\x1b]4;0;?\x07".as_slice(),
        b"\x1b]10;?\x07".as_slice(),
        b"\x1b]11;?\x07",
        b"\x1b]12;?\x07",
        b"\x1b[6n",
    ]
    .concat();
    let expected = [
        INDEXED_BLACK_REPLY,
        FOREGROUND_COLOR_REPLY,
        BACKGROUND_COLOR_REPLY,
        CURSOR_COLOR_REPLY,
        b"\x1b[1;1R".as_slice(),
    ]
    .concat();

    for chunk_size in [input.len(), 1, 3, 5] {
        let chunks = input
            .chunks(chunk_size)
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        let (replies, _) = run_ssh_output(&chunks);
        assert_eq!(
            replies, expected,
            "chunk size {chunk_size}: each color query has one model reply"
        );
    }
}

#[test]
fn color_queries_keep_their_stream_position_inside_a_synchronized_update() {
    let input = [
        b"\x1b[?2026h".as_slice(),
        b"\x1b]10;?\x07",
        b"\x1b[c",
        b"\x1b]11;?\x07",
        b"\x1b[?2026l",
    ]
    .concat();
    let expected = [
        FOREGROUND_COLOR_REPLY,
        SSH_DA1_REPLY,
        BACKGROUND_COLOR_REPLY,
    ]
    .concat();

    for chunk_size in [input.len(), 1, 2, 7] {
        let chunks = input
            .chunks(chunk_size)
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        let (replies, _) = run_ssh_output(&chunks);
        assert_eq!(
            replies, expected,
            "chunk size {chunk_size}: color and fixed replies keep stream order"
        );
    }
}

#[test]
fn an_unclosed_synchronized_update_is_flushed_at_its_deadline() {
    let (model, state) = state(true, 64);
    let (pipeline, replies) = start_ssh_pipeline(state);
    let metrics = pipeline.queue.metrics_handle();
    let hidden = b"\x1b[?2026hHELD\x1b[4;7H\x1b[6n\x1b[c";
    pipeline
        .queue
        .send(hidden.to_vec())
        .expect("queue an open synchronized update");
    assert!(
        wait_until(|| metrics.acknowledged_bytes() == hidden.len() as u64),
        "the parser consumes the open block"
    );
    assert!(!text(&model).contains("HELD"));
    assert!(
        replies.lock().expect("captured replies").is_empty(),
        "nothing is answered before the deadline"
    );

    let expected = [b"\x1b[4;7R".as_slice(), SSH_DA1_REPLY].concat();
    assert!(
        wait_until(|| *replies.lock().expect("captured replies") == expected),
        "the parser aborts the unclosed update and answers what it held"
    );
    assert!(!model.has_synchronized_output(), "the update is over");
    assert!(text(&model).contains("HELD"), "the held text is now shown");

    let tail = b"\r\nAFTER\x1b[c";
    pipeline
        .queue
        .send(tail.to_vec())
        .expect("queue output after the aborted update");
    assert!(
        wait_until(|| text(&model).contains("AFTER")),
        "later output is parsed normally instead of staying buffered"
    );
    let after_abort = [expected.as_slice(), SSH_DA1_REPLY].concat();
    assert!(
        wait_until(|| *replies.lock().expect("captured replies") == after_abort),
        "a fixed reply after the abort is written once, without the stale block"
    );
    assert_eq!(
        *replies.lock().expect("captured replies"),
        after_abort,
        "the aborted block does not answer again"
    );
    drop(pipeline.queue);
    pipeline.parser.join().expect("parser exits");
}

#[test]
fn marker_aliases_and_cancellations_keep_their_real_reports() {
    let mode_report = b"\x1b[?57344;0$y";
    let input = [
        b"\x1b[?2026h".as_slice(),
        b"\x1b[4;7H\x1b[6n",
        b"\x1b[?057344$p",
        b"\x1b[?57344$\x18p",
        b"\x1b[?57344;7$p",
        b"\x1b[c",
        b"\x1b[?57344$p",
        b"\x1b[?2026l",
        b"\x1b[2;5H\x1b[6n",
    ]
    .concat();
    let expected = [
        b"\x1b[4;7R".as_slice(),
        mode_report,
        mode_report,
        SSH_DA1_REPLY,
        mode_report,
        b"\x1b[2;5R",
    ]
    .concat();

    for chunk_size in [input.len(), 1, 2, 3, 7, 11, 16] {
        let chunks = input
            .chunks(chunk_size)
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        let (replies, _) = run_ssh_output(&chunks);
        assert_eq!(
            replies, expected,
            "chunk size {chunk_size}: aliases and cancelled queries keep the model's report order"
        );
    }
}

#[test]
fn extending_a_sync_block_keeps_earlier_private_reports_in_place() {
    let input = b"\x1b[?2026h\x1b[?57344$p\x1b[?2026h\x1b[c\x1b[?2026l";
    let expected = [b"\x1b[?57344;0$y".as_slice(), SSH_DA1_REPLY].concat();
    for size in [1, 7, input.len()] {
        let chunks = input.chunks(size).map(<[u8]>::to_vec).collect::<Vec<_>>();
        assert_eq!(run_ssh_output(&chunks).0, expected, "chunk size {size}");
    }
}

#[test]
fn a_new_sync_block_in_the_same_chunk_keeps_pending_reports_in_place() {
    let input = [
        b"\x1b[?2026h".as_slice(),
        b"\x1b[?57344$p",
        b"\x1b[?2026h",
        b"\x1b[c",
        b"\x1b[?2026l",
        b"\x1b[?2026h",
        b"\x1b[?57344$p",
        b"\x1b[c",
        b"\x1b[?2026l",
    ]
    .concat();
    let mode_report = b"\x1b[?57344;0$y";
    let expected = [mode_report, SSH_DA1_REPLY, mode_report, SSH_DA1_REPLY].concat();
    let shared_chunk = [
        b"\x1b[?2026h\x1b[?57344$p\x1b[?2026h".to_vec(),
        b"\x1b[c\x1b[?2026l\x1b[?2026h\x1b[?57344$p".to_vec(),
        b"\x1b[c\x1b[?2026l".to_vec(),
    ];
    assert_eq!(
        run_ssh_output(&shared_chunk).0,
        expected,
        "an ESU and a new BSU in one chunk keep both reports in place"
    );
    for size in [1, 7, 11, input.len()] {
        let chunks = input.chunks(size).map(<[u8]>::to_vec).collect::<Vec<_>>();
        assert_eq!(run_ssh_output(&chunks).0, expected, "chunk size {size}");
    }
}
