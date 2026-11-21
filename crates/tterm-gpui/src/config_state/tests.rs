//! Configuration state tests that cross the real Tokio blocking worker.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{Entity, Task, TestAppContext};
use tterm_core::config::AppConfig;

use super::{ConfigState, ScrollbackChanged, ScrollbackWriter};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime")
}

fn lines(cx: &TestAppContext, state: &Entity<ConfigState>) -> u32 {
    cx.read(|app| state.read(app).scrollback_lines())
}

fn save_pending(cx: &TestAppContext, state: &Entity<ConfigState>) -> bool {
    cx.read(|app| state.read(app).is_save_pending())
}

fn subscribe(cx: &mut TestAppContext, state: &Entity<ConfigState>) -> Arc<Mutex<Vec<u32>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    cx.update(|cx| {
        cx.subscribe(state, move |_, event: &ScrollbackChanged, _| {
            recorded.lock().expect("events").push(event.lines);
        })
        .detach();
    });
    events
}

fn save(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
    lines: u32,
) -> Task<Result<u32, String>> {
    cx.update(|cx| state.update(cx, |state, cx| state.save_scrollback_lines(lines, cx)))
}

type WriterLatch = Arc<(Mutex<bool>, Condvar)>;

/// Releases a blocked writer when it drops, so a failed assertion cannot hang
/// the Tokio runtime while it waits for blocking tasks to finish.
struct ReleaseOnDrop(WriterLatch);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        release(&self.0);
    }
}

fn release(latch: &WriterLatch) {
    let (lock, condvar) = &**latch;
    *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    condvar.notify_all();
}

fn blocking_writer(entered: mpsc::Sender<()>, latch: WriterLatch) -> ScrollbackWriter {
    Arc::new(move |_| {
        let _ = entered.send(());
        let (lock, condvar) = &*latch;
        let mut released = lock
            .lock()
            .map_err(|_| "the writer latch was poisoned".to_string())?;
        while !*released {
            released = condvar
                .wait(released)
                .map_err(|_| "the writer latch was poisoned".to_string())?;
        }
        Ok(())
    })
}

#[gpui_kit::test]
async fn reads_the_snapshot_loaded_before_the_application_started(cx: &mut TestAppContext) {
    let runtime = runtime();
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writer(
                AppConfig {
                    scrollback_lines: 12_345,
                    ..AppConfig::default()
                },
                runtime.handle(),
                Arc::new(|_| Ok(())),
            )
        })
    });

    assert_eq!(lines(cx, &state), 12_345);
    assert_eq!(
        cx.read(|app| state.read(app).config().scrollback_lines),
        12_345
    );
}

#[gpui_kit::test]
async fn saves_the_normalized_value_then_emits_the_change(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let writer: ScrollbackWriter = Arc::new(move |lines| {
        recorded.lock().expect("writes").push(lines);
        Ok(())
    });
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_writer(AppConfig::default(), runtime.handle(), writer))
    });
    let events = subscribe(cx, &state);
    cx.executor().allow_parking();

    let task = save(cx, &state, 20_000_000);
    let result = task.await;

    assert_eq!(result, Ok(10_000_000));
    assert_eq!(*writes.lock().expect("writes"), vec![10_000_000]);
    assert_eq!(lines(cx, &state), 10_000_000);
    assert_eq!(*events.lock().expect("events"), vec![10_000_000]);
}

#[gpui_kit::test]
async fn keeps_the_saved_value_when_the_write_fails_and_remains_usable(cx: &mut TestAppContext) {
    let runtime = runtime();
    let fail = Arc::new(AtomicUsize::new(1));
    let writer_fail = Arc::clone(&fail);
    let writer: ScrollbackWriter = Arc::new(move |_| {
        if writer_fail.fetch_sub(1, Ordering::SeqCst) > 0 {
            Err("disk is full".to_string())
        } else {
            Ok(())
        }
    });
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_writer(AppConfig::default(), runtime.handle(), writer))
    });
    let events = subscribe(cx, &state);
    cx.executor().allow_parking();

    let task = save(cx, &state, 0);
    let result = task.await;

    assert_eq!(result, Err("disk is full".to_string()));
    assert_eq!(lines(cx, &state), 10_000);
    assert!(events.lock().expect("events").is_empty());
    assert!(!save_pending(cx, &state));

    let task = save(cx, &state, 0);
    let result = task.await;
    assert_eq!(result, Ok(0));
    assert_eq!(lines(cx, &state), 0);
    assert_eq!(*events.lock().expect("events"), vec![0]);
}

#[gpui_kit::test]
async fn refuses_an_overlapping_save_without_changing_the_setting(cx: &mut TestAppContext) {
    let runtime = runtime();
    let (entered_tx, entered_rx) = mpsc::channel();
    let latch: WriterLatch = Arc::new((Mutex::new(false), Condvar::new()));
    let release_on_drop = ReleaseOnDrop(Arc::clone(&latch));
    let writer = blocking_writer(entered_tx, Arc::clone(&latch));
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_writer(AppConfig::default(), runtime.handle(), writer))
    });
    let events = subscribe(cx, &state);
    cx.executor().allow_parking();

    let first = save(cx, &state, 20_000);
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the writer entered its blocking section");

    let second = save(cx, &state, 30_000).await;

    assert!(second.is_err(), "a second edit waits for the first save");
    assert_eq!(lines(cx, &state), 10_000);
    assert!(events.lock().expect("events").is_empty());

    release(&latch);
    assert_eq!(first.await, Ok(20_000));
    assert_eq!(lines(cx, &state), 20_000);
    assert_eq!(*events.lock().expect("events"), vec![20_000]);

    drop(release_on_drop);
}

#[gpui_kit::test]
async fn a_dropped_waiter_still_commits_the_saved_value(cx: &mut TestAppContext) {
    let runtime = runtime();
    let (entered_tx, entered_rx) = mpsc::channel();
    let latch: WriterLatch = Arc::new((Mutex::new(false), Condvar::new()));
    let release_on_drop = ReleaseOnDrop(Arc::clone(&latch));
    let writer = blocking_writer(entered_tx, Arc::clone(&latch));
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_writer(AppConfig::default(), runtime.handle(), writer))
    });
    let events = subscribe(cx, &state);
    cx.executor().allow_parking();

    let first = save(cx, &state, 20_000);
    drop(first);
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the writer entered its blocking section");

    release(&latch);
    wait_until(cx, |cx| !save_pending(cx, &state)).await;

    assert_eq!(lines(cx, &state), 20_000);
    assert_eq!(*events.lock().expect("events"), vec![20_000]);

    let second = save(cx, &state, 0).await;
    assert_eq!(second, Ok(0));
    assert_eq!(lines(cx, &state), 0);
    assert_eq!(*events.lock().expect("events"), vec![20_000, 0]);

    drop(release_on_drop);
}

#[gpui_kit::test]
async fn an_unchanged_setting_skips_the_write_and_the_event(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writes = Arc::new(AtomicUsize::new(0));
    let writer_writes = Arc::clone(&writes);
    let writer: ScrollbackWriter = Arc::new(move |_| {
        writer_writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_writer(AppConfig::default(), runtime.handle(), writer))
    });
    let events = subscribe(cx, &state);
    cx.executor().allow_parking();

    let task = save(cx, &state, 10_000);
    let result = task.await;

    assert_eq!(result, Ok(10_000));
    assert_eq!(writes.load(Ordering::SeqCst), 0);
    assert!(events.lock().expect("events").is_empty());
}

async fn wait_until(cx: &TestAppContext, mut condition: impl FnMut(&TestAppContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition(cx) {
        assert!(Instant::now() < deadline, "timed out waiting for the save");
        cx.executor().timer(Duration::from_millis(10)).await;
    }
}
