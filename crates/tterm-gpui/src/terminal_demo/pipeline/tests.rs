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
use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

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

fn state(visible: bool, scrollback: usize) -> (Arc<Terminal>, SessionState) {
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
        session_nonce: 1,
        alive: Arc::new(AtomicBool::new(true)),
        visible: Arc::new(AtomicBool::new(visible)),
        refresh: Arc::new(RefreshSignal::new()),
    };
    (terminal, state)
}

fn start_pipeline(
    state: SessionState,
    acknowledge: impl Fn(usize) + Send + Sync + 'static,
) -> Pipeline {
    start(state, Arc::new(|_| {}), Arc::new(acknowledge)).expect("start the parser")
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
    std::thread::sleep(Duration::from_millis(100));

    foreground.reset_snapshot_timing();
    let loaded = measure_frames(cx, handle, &foreground, 120);
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
    assert!(
        acknowledged >= 30 * 1024 * 1024,
        "the measurement processed only {acknowledged} bytes across 30 background sessions"
    );
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
    eprintln!(
        "P2-02 foreground frames: baseline p50={baseline_p50:?} p95={baseline_p95:?}; \
         30-background p50={loaded_p50:?} p95={loaded_p95:?}; snapshots={} frames={}; \
         bytes={acknowledged}; snapshot_wait={:?} total, \
         snapshot_hold={:?} total / {:?} max over {} snapshots",
        stats.snapshots,
        stats.frames,
        timing.waited,
        timing.held,
        timing.held_max,
        timing.snapshots,
    );
    assert!(
        loaded_p95 <= Duration::from_millis(16) + Duration::from_micros(667),
        "30 background parsers pushed the foreground p95 frame time to {loaded_p95:?}"
    );
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
