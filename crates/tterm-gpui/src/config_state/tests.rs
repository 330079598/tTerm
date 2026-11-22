//! Configuration state tests that cross the real Tokio blocking worker.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{Entity, Task, TestAppContext};
use serde_json::json;
use tterm_core::config::AppConfig;
use tterm_core::config::panes::{PaneConfig, PaneLayoutTemplate};

use super::{
    ConfigState, PaneTemplatesChanged, PaneTemplatesWriter, ScrollbackChanged, ScrollbackWriter,
};

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

fn template(id: &str, first: f64, second: f64) -> PaneLayoutTemplate {
    PaneLayoutTemplate {
        id: id.to_string(),
        name: format!("Layout {id}"),
        root: json!({
            "type": "split",
            "orientation": "horizontal",
            "children": [{"type": "pane"}, {"type": "pane"}],
            "sizes": [first, second]
        }),
    }
}

fn templates(cx: &TestAppContext, state: &Entity<ConfigState>) -> Vec<PaneLayoutTemplate> {
    cx.read(|app| state.read(app).config().panes.templates.clone())
}

fn subscribe_templates(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
) -> Arc<Mutex<Vec<Vec<PaneLayoutTemplate>>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    cx.update(|cx| {
        cx.subscribe(state, move |_, event: &PaneTemplatesChanged, _| {
            recorded
                .lock()
                .expect("template events")
                .push(event.templates.clone());
        })
        .detach();
    });
    events
}

fn save_templates(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
    templates: Vec<PaneLayoutTemplate>,
) -> Task<Result<(), String>> {
    cx.update(|cx| state.update(cx, |state, cx| state.save_pane_templates(templates, cx)))
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

fn blocking_templates_writer(entered: mpsc::Sender<()>, latch: WriterLatch) -> PaneTemplatesWriter {
    Arc::new(move |_| {
        let _ = entered.send(());
        let (lock, condvar) = &*latch;
        let mut released = lock
            .lock()
            .map_err(|_| "the template writer latch was poisoned".to_string())?;
        while !*released {
            released = condvar
                .wait(released)
                .map_err(|_| "the template writer latch was poisoned".to_string())?;
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

#[gpui_kit::test]
async fn saves_the_complete_template_list_then_publishes_and_emits_it(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let writer: PaneTemplatesWriter = Arc::new(move |templates| {
        recorded.lock().expect("template writes").push(templates);
        Ok(())
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writers(
                AppConfig::default(),
                runtime.handle(),
                Arc::new(|_| Ok(())),
                writer,
            )
        })
    });
    let events = subscribe_templates(cx, &state);
    cx.executor().allow_parking();
    let saved = vec![template("one", 0.4, 0.6), template("two", 0.25, 0.75)];

    let result = save_templates(cx, &state, saved.clone()).await;

    assert_eq!(result, Ok(()));
    assert_eq!(
        *writes.lock().expect("template writes"),
        vec![saved.clone()]
    );
    assert_eq!(templates(cx, &state), saved);
    assert_eq!(*events.lock().expect("template events"), vec![saved]);
}

#[gpui_kit::test]
async fn publishes_the_canonical_shape_that_reaches_disk(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let writer: PaneTemplatesWriter = Arc::new(move |templates| {
        recorded.lock().expect("template writes").push(templates);
        Ok(())
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writers(
                AppConfig::default(),
                runtime.handle(),
                Arc::new(|_| Ok(())),
                writer,
            )
        })
    });
    let events = subscribe_templates(cx, &state);
    cx.executor().allow_parking();
    let input = vec![PaneLayoutTemplate {
        id: "ratios".to_string(),
        name: "Layout ratios".to_string(),
        root: json!({
            "type": "split",
            "orientation": "horizontal",
            "children": [{"type": "pane"}, {"type": "pane"}],
            "sizes": [3, 1]
        }),
    }];

    assert_eq!(save_templates(cx, &state, input).await, Ok(()));

    let expected = vec![template("ratios", 0.75, 0.25)];
    assert_eq!(
        *writes.lock().expect("template writes"),
        vec![expected.clone()]
    );
    assert_eq!(templates(cx, &state), expected);
    assert_eq!(*events.lock().expect("template events"), vec![expected]);
}

#[gpui_kit::test]
async fn keeps_the_old_templates_when_the_write_fails_and_remains_usable(cx: &mut TestAppContext) {
    let runtime = runtime();
    let old = vec![template("old", 0.5, 0.5)];
    let fail = Arc::new(AtomicUsize::new(1));
    let writer_fail = Arc::clone(&fail);
    let writer: PaneTemplatesWriter = Arc::new(move |_| {
        if writer_fail.fetch_sub(1, Ordering::SeqCst) > 0 {
            Err("disk is full".to_string())
        } else {
            Ok(())
        }
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writers(
                AppConfig {
                    panes: PaneConfig {
                        templates: old.clone(),
                        ..PaneConfig::default()
                    },
                    ..AppConfig::default()
                },
                runtime.handle(),
                Arc::new(|_| Ok(())),
                writer,
            )
        })
    });
    let events = subscribe_templates(cx, &state);
    cx.executor().allow_parking();
    let next = vec![template("next", 0.3, 0.7)];

    let result = save_templates(cx, &state, next.clone()).await;

    assert_eq!(result, Err("disk is full".to_string()));
    assert_eq!(templates(cx, &state), old);
    assert!(events.lock().expect("template events").is_empty());
    assert!(!save_pending(cx, &state));

    let result = save_templates(cx, &state, next.clone()).await;
    assert_eq!(result, Ok(()));
    assert_eq!(templates(cx, &state), next);
    assert_eq!(*events.lock().expect("template events"), vec![next]);
}

#[gpui_kit::test]
async fn refuses_template_edits_while_a_scrollback_save_is_pending(cx: &mut TestAppContext) {
    let runtime = runtime();
    let (entered_tx, entered_rx) = mpsc::channel();
    let latch: WriterLatch = Arc::new((Mutex::new(false), Condvar::new()));
    let release_on_drop = ReleaseOnDrop(Arc::clone(&latch));
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writers(
                AppConfig::default(),
                runtime.handle(),
                blocking_writer(entered_tx, Arc::clone(&latch)),
                Arc::new(|_| Ok(())),
            )
        })
    });
    let events = subscribe_templates(cx, &state);
    cx.executor().allow_parking();

    let scrollback = save(cx, &state, 20_000);
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the scrollback writer entered its blocking section");
    let result = save_templates(cx, &state, vec![template("blocked", 0.5, 0.5)]).await;

    assert!(
        result.is_err(),
        "a template edit cannot start from a stale snapshot"
    );
    assert!(templates(cx, &state).is_empty());
    assert!(events.lock().expect("template events").is_empty());

    release(&latch);
    assert_eq!(scrollback.await, Ok(20_000));
    drop(release_on_drop);
}

#[gpui_kit::test]
async fn a_dropped_template_waiter_still_commits_and_releases_pending(cx: &mut TestAppContext) {
    let runtime = runtime();
    let (entered_tx, entered_rx) = mpsc::channel();
    let latch: WriterLatch = Arc::new((Mutex::new(false), Condvar::new()));
    let release_on_drop = ReleaseOnDrop(Arc::clone(&latch));
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writers(
                AppConfig::default(),
                runtime.handle(),
                Arc::new(|_| Ok(())),
                blocking_templates_writer(entered_tx, Arc::clone(&latch)),
            )
        })
    });
    let events = subscribe_templates(cx, &state);
    cx.executor().allow_parking();
    let saved = vec![template("dropped", 0.45, 0.55)];

    let first = save_templates(cx, &state, saved.clone());
    drop(first);
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the template writer entered its blocking section");

    release(&latch);
    wait_until(cx, |cx| !save_pending(cx, &state)).await;

    assert_eq!(templates(cx, &state), saved);
    assert_eq!(*events.lock().expect("template events"), vec![saved]);
    let second = save_templates(cx, &state, Vec::new()).await;
    assert_eq!(second, Ok(()));
    assert!(templates(cx, &state).is_empty());

    drop(release_on_drop);
}

async fn wait_until(cx: &TestAppContext, mut condition: impl FnMut(&TestAppContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition(cx) {
        assert!(Instant::now() < deadline, "timed out waiting for the save");
        cx.executor().timer(Duration::from_millis(10)).await;
    }
}
