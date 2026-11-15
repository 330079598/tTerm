//! Window geometry and persistence tests.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use gpui_kit::{
    AppContext as _, Bounds, Context, Entity, IntoElement, Render, TestAppContext, Window,
    WindowBounds, div, point, px, size,
};
use tterm_core::db::Database;

use super::{
    KEY, Observation, SaveQueue, SavedWindowState, StateWriter, WindowGeometry, fold, read,
    resolve_window_bounds, shared_latest, write,
};
use crate::app_window::logical_bounds::{LogicalArea, LogicalBounds};

fn logical(width: f32, height: f32) -> LogicalBounds {
    LogicalBounds {
        x: 100.0,
        y: 50.0,
        width,
        height,
    }
}

fn saved(width: f32, height: f32, maximized: bool) -> SavedWindowState {
    SavedWindowState {
        bounds: logical(width, height),
        maximized,
    }
}

#[test]
fn state_round_trips_through_app_meta() {
    let database = Database::open_in_memory().expect("open in-memory database");
    let state = saved(1200.0, 800.0, true);
    database
        .write(|connection| write(connection, state))
        .expect("write state");
    let restored = database.read(read).expect("read state");
    assert_eq!(restored, Some(state));
    assert_eq!(KEY, "gpui_window_state");
    let json = serde_json::to_string(&state).expect("serialize state");
    assert!(json.contains("\"bounds\""));
    assert!(json.contains("\"maximized\""));
    assert!(
        !json.contains("scale"),
        "logical state stores no guessed scale"
    );
}

#[test]
fn a_windowed_observation_replaces_the_saved_bounds() {
    let observed = Observation::Windowed(logical(1000.0, 700.0));
    let result = fold(None, Some(observed)).expect("windowed state");
    assert!(!result.maximized);
    assert_eq!(result.bounds.width, 1000.0);
    assert_eq!(result.bounds.height, 700.0);
}

#[test]
fn a_maximized_observation_keeps_the_previous_restore_bounds() {
    let previous = saved(1000.0, 700.0, false);
    let result = fold(
        Some(previous),
        Some(Observation::Maximized(logical(1920.0, 1080.0))),
    )
    .expect("maximized state");
    assert!(result.maximized);
    assert_eq!(result.bounds, previous.bounds);
}

#[test]
fn fullscreen_and_zero_size_observations_keep_the_previous_state() {
    let previous = saved(1000.0, 700.0, false);
    assert_eq!(
        fold(Some(previous), Some(Observation::Fullscreen)),
        Some(previous)
    );
    assert_eq!(
        fold(
            Some(previous),
            Some(Observation::Windowed(logical(0.0, 700.0))),
        ),
        Some(previous)
    );
}

#[test]
fn off_screen_bounds_are_centered_on_the_primary_monitor() {
    let state = SavedWindowState {
        bounds: LogicalBounds {
            x: 5000.0,
            y: 100.0,
            width: 800.0,
            height: 600.0,
        },
        maximized: false,
    };
    let monitor = LogicalArea {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    let fallback = Bounds {
        origin: point(px(0.0), px(0.0)),
        size: size(px(1200.0), px(800.0)),
    };
    let restored = resolve_window_bounds(state, &[monitor], Some(monitor), fallback);
    let WindowBounds::Windowed(bounds) = restored else {
        panic!("windowed state expected");
    };
    assert_eq!(bounds.size.width.as_f32(), 800.0);
    assert_eq!(bounds.origin.x.as_f32(), 560.0);
}

#[test]
fn negative_monitor_origins_are_valid_placement() {
    let state = SavedWindowState {
        bounds: LogicalBounds {
            x: -1500.0,
            y: 100.0,
            width: 800.0,
            height: 600.0,
        },
        maximized: false,
    };
    let left_monitor = LogicalArea {
        x: -1920.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    let fallback = Bounds {
        origin: point(px(0.0), px(0.0)),
        size: size(px(1200.0), px(800.0)),
    };
    let restored = resolve_window_bounds(state, &[left_monitor], Some(left_monitor), fallback);
    let WindowBounds::Windowed(bounds) = restored else {
        panic!("windowed state expected");
    };
    assert_eq!(bounds.origin.x.as_f32(), -1500.0);
}

#[test]
fn the_shared_slot_returns_the_last_value() {
    let shared = Arc::new(Mutex::new(None));
    assert_eq!(shared_latest(&shared), None);
    let state = saved(800.0, 600.0, false);
    *shared.lock().expect("shared state") = Some(state);
    assert_eq!(shared_latest(&shared), Some(state));
}

#[test]
fn save_queue_writes_in_flush_order() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime");
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let queue = SaveQueue::start(
        runtime.handle(),
        Arc::new(move |state| {
            recorded.lock().expect("writes").push(state.bounds);
            Ok(())
        }),
    );
    let first = saved(800.0, 600.0, false);
    let second = saved(1000.0, 700.0, true);
    runtime.block_on(async {
        queue.flush(first).await.expect("first save");
        queue.flush(second).await.expect("second save");
    });
    assert_eq!(
        *writes.lock().expect("writes"),
        vec![first.bounds, second.bounds]
    );
}

#[test]
fn save_queue_propagates_a_writer_error() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime");
    let queue = SaveQueue::start(
        runtime.handle(),
        Arc::new(|_| Err("disk is full".to_string())),
    );
    let result = runtime.block_on(queue.flush(saved(800.0, 600.0, false)));
    assert_eq!(result, Err("disk is full".to_string()));
}

#[gpui_kit::test]
fn close_gate_waits_for_the_blocked_writer_and_ignores_duplicates(cx: &mut TestAppContext) {
    // This regression intentionally crosses the real Tokio blocking pool.
    cx.executor().allow_parking();
    struct Harness {
        geometry: Entity<WindowGeometry>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime");
    let (entered_tx, entered_rx) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    struct ReleaseOnDrop(Arc<(Mutex<bool>, Condvar)>);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            let (lock, condvar) = &*self.0;
            *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
            condvar.notify_all();
        }
    }
    // The runtime waits for blocking tasks during drop, including after a failed assertion.
    let release_on_drop = ReleaseOnDrop(Arc::clone(&release));
    let writes = Arc::new(AtomicUsize::new(0));
    let writer_release = Arc::clone(&release);
    let writer_writes = Arc::clone(&writes);
    let writer: StateWriter = Arc::new(move |_state| {
        writer_writes.fetch_add(1, Ordering::SeqCst);
        let _ = entered_tx.send(());
        let (lock, condvar) = &*writer_release;
        let mut released = lock
            .lock()
            .map_err(|_| "writer latch poisoned".to_string())?;
        while !*released {
            released = condvar
                .wait(released)
                .map_err(|_| "writer latch poisoned".to_string())?;
        }
        Ok(())
    });
    let queue = SaveQueue::start(runtime.handle(), writer);
    let shared = Arc::new(Mutex::new(None));
    let (view, vcx) = cx.add_window_view(|_, cx| {
        let geometry = cx.new(|_| WindowGeometry::new(None, Arc::clone(&shared), queue.clone()));
        Harness { geometry }
    });
    let geometry = vcx.update(|_, app| view.read(app).geometry.clone());
    vcx.update(|window, app| {
        geometry.update(app, |geometry, cx| geometry.attach(window, cx));
    });
    let first = vcx.simulate_close();
    assert!(!first, "the first close is held open for the flush");
    let second = vcx.simulate_close();
    assert!(
        !second,
        "a duplicate close is refused while the flush is in flight"
    );
    assert_eq!(vcx.cx.windows().len(), 1, "the window stays open");

    vcx.cx.run_until_parked();
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the writer entered its blocking section");
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(
        vcx.cx.windows().len(),
        1,
        "the blocked writer keeps the window"
    );
    assert_eq!(
        writes.load(Ordering::SeqCst),
        1,
        "only one write was scheduled"
    );

    drop(release_on_drop);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !vcx.cx.windows().is_empty() && Instant::now() < deadline {
        vcx.cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        vcx.cx.windows().is_empty(),
        "the window closes after the flush"
    );
    assert_eq!(writes.load(Ordering::SeqCst), 1);
}
