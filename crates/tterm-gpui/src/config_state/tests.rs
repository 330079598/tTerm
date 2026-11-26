//! Configuration state tests that cross the real Tokio blocking worker.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{Entity, Task, TestAppContext};
use serde_json::json;
use tterm_core::config::AppConfig;
use tterm_core::config::local_shells::LocalShell;
use tterm_core::config::local_shells_file::{SavedLocalShells, SavedShellLists, ShellListEdit};
use tterm_core::config::panes::{PaneConfig, PaneLayoutTemplate};
use tterm_core::config::terminal_font_file::SavedTerminalFont;
use tterm_core::terminal::pty::TerminalShellProfile;
use tterm_core::workspace::local_shells::default_shell::default_shell_updates;
use tterm_core::workspace::local_shells::types::{DefaultShellSettings, LocalShellChoice};

use super::{
    ConfigState, DefaultShellWriter, LocalShellsChanged, LocalShellsWriter, PaneTemplatesChanged,
    PaneTemplatesWriter, ScrollbackChanged, ScrollbackWriter, ShellListWriter, TerminalFontChanged,
    TerminalFontWriter, echo_local_shells_writer,
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
                echo_local_shells_writer(),
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
                echo_local_shells_writer(),
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
                echo_local_shells_writer(),
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
                echo_local_shells_writer(),
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
                echo_local_shells_writer(),
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

fn local_shell(id: &str, path: &str) -> LocalShell {
    LocalShell {
        id: id.to_string(),
        name: id.to_string(),
        path: path.to_string(),
        args: String::new(),
    }
}

fn local_shells(cx: &TestAppContext, state: &Entity<ConfigState>) -> Vec<LocalShell> {
    cx.read(|app| state.read(app).config().local_shells.clone())
}

fn save_local_shell(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
    shell: LocalShell,
) -> Task<Result<LocalShellsChanged, String>> {
    cx.update(|cx| state.update(cx, |state, cx| state.save_local_shell(shell, cx)))
}

fn subscribe_local_shells(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
) -> Arc<Mutex<Vec<Vec<LocalShell>>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    cx.update(|cx| {
        cx.subscribe(state, move |_, event: &LocalShellsChanged, _| {
            recorded
                .lock()
                .expect("local shell events")
                .push(event.local_shells.clone());
        })
        .detach();
    });
    events
}

fn default_shell(cx: &TestAppContext, state: &Entity<ConfigState>) -> DefaultShellSettings {
    cx.read(|app| {
        let config = state.read(app).config();
        DefaultShellSettings {
            terminal_shell: config.terminal_shell.clone(),
            terminal_shell_custom_path: config.terminal_shell_custom_path.clone(),
            terminal_shell_custom_args: config.terminal_shell_custom_args.clone(),
            terminal_shell_id: config.terminal_shell_id.clone(),
            terminal_shell_wsl_distro: config.terminal_shell_wsl_distro.clone(),
        }
    })
}

fn subscribe_default_shells(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
) -> Arc<Mutex<Vec<DefaultShellSettings>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    cx.update(|cx| {
        cx.subscribe(state, move |_, event: &LocalShellsChanged, _| {
            recorded
                .lock()
                .expect("default shell events")
                .push(event.default_shell.clone());
        })
        .detach();
    });
    events
}

fn own_shell_choice(id: &str, path: &str, args: &str) -> LocalShellChoice {
    LocalShellChoice {
        shell: "custom".to_string(),
        custom_path: Some(path.to_string()),
        custom_args: (!args.is_empty()).then(|| args.to_string()),
        shell_id: Some(id.to_string()),
        wsl_distro: None,
    }
}

fn set_default_shell(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
    choice: LocalShellChoice,
) -> Task<Result<LocalShellsChanged, String>> {
    cx.update(|cx| state.update(cx, |state, cx| state.set_default_shell(choice, cx)))
}

fn edit_shell_list(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
    edit: ShellListEdit,
) -> Task<Result<LocalShellsChanged, String>> {
    cx.update(|cx| state.update(cx, |state, cx| state.edit_shell_list(edit, cx)))
}

fn hidden_shells(cx: &TestAppContext, state: &Entity<ConfigState>) -> Vec<String> {
    cx.read(|app| state.read(app).config().hidden_shells.clone())
}

/// The visible shells, hidden keys and default choice each published event
/// carries.
type RecordedShellListEvents =
    Arc<Mutex<Vec<(Vec<LocalShell>, Vec<String>, DefaultShellSettings)>>>;

fn subscribe_shell_lists(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
) -> RecordedShellListEvents {
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    cx.update(|cx| {
        cx.subscribe(state, move |_, event: &LocalShellsChanged, _| {
            recorded.lock().expect("shell list events").push((
                event.local_shells.clone(),
                event.hidden_shells.clone(),
                event.default_shell.clone(),
            ));
        })
        .detach();
    });
    events
}

#[gpui_kit::test]
async fn saves_a_local_shell_then_publishes_the_latest_list(cx: &mut TestAppContext) {
    let runtime = runtime();
    let existing = local_shell("existing", "/bin/zsh");
    let writer_existing = existing.clone();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let writer: LocalShellsWriter = Arc::new(move |shell| {
        recorded
            .lock()
            .expect("local shell writes")
            .push(shell.clone());
        Ok(SavedLocalShells {
            local_shells: vec![writer_existing.clone(), shell],
            default_shell: DefaultShellSettings::default(),
        })
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writers(
                AppConfig {
                    local_shells: vec![existing.clone()],
                    ..AppConfig::default()
                },
                runtime.handle(),
                Arc::new(|_| Ok(())),
                Arc::new(|_| Ok(())),
                writer,
            )
        })
    });
    let events = subscribe_local_shells(cx, &state);
    cx.executor().allow_parking();
    let added = local_shell("added", "/bin/nu");

    let result = save_local_shell(cx, &state, added.clone()).await;

    let saved = result.expect("the save reports the list");
    assert_eq!(saved.local_shells, vec![existing.clone(), added.clone()]);
    assert_eq!(local_shells(cx, &state), vec![existing, added.clone()]);
    assert_eq!(
        *writes.lock().expect("local shell writes"),
        vec![added.clone()]
    );
    assert_eq!(
        *events.lock().expect("local shell events"),
        vec![saved.local_shells]
    );
}

#[gpui_kit::test]
async fn keeps_the_old_local_shells_when_the_write_fails_and_remains_usable(
    cx: &mut TestAppContext,
) {
    let runtime = runtime();
    let existing = local_shell("existing", "/bin/zsh");
    let fail = Arc::new(AtomicUsize::new(1));
    let writer_fail = Arc::clone(&fail);
    let writer: LocalShellsWriter = Arc::new(move |shell| {
        if writer_fail.fetch_sub(1, Ordering::SeqCst) > 0 {
            Err("disk is full".to_string())
        } else {
            Ok(SavedLocalShells {
                local_shells: vec![shell],
                default_shell: DefaultShellSettings::default(),
            })
        }
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writers(
                AppConfig {
                    local_shells: vec![existing.clone()],
                    ..AppConfig::default()
                },
                runtime.handle(),
                Arc::new(|_| Ok(())),
                Arc::new(|_| Ok(())),
                writer,
            )
        })
    });
    let events = subscribe_local_shells(cx, &state);
    cx.executor().allow_parking();
    let added = local_shell("added", "/bin/nu");

    let result = save_local_shell(cx, &state, added.clone()).await;

    assert_eq!(result, Err("disk is full".to_string()));
    assert_eq!(local_shells(cx, &state), vec![existing]);
    assert!(events.lock().expect("local shell events").is_empty());
    assert!(!save_pending(cx, &state));

    let result = save_local_shell(cx, &state, added.clone()).await;
    assert!(result.is_ok());
    assert_eq!(local_shells(cx, &state), vec![added]);
    assert_eq!(
        events.lock().expect("local shell events").len(),
        1,
        "only the successful save emits an event"
    );
}

#[gpui_kit::test]
async fn saves_the_default_shell_then_publishes_the_latest_list(cx: &mut TestAppContext) {
    let runtime = runtime();
    let existing = local_shell("nu", "/bin/nu");
    let writer_existing = existing.clone();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let writer: DefaultShellWriter = Arc::new(move |choice: LocalShellChoice| {
        recorded
            .lock()
            .expect("default shell writes")
            .push(choice.clone());
        Ok(SavedLocalShells {
            local_shells: vec![writer_existing.clone()],
            default_shell: default_shell_updates(&choice),
        })
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_default_shell_writer(
                AppConfig {
                    local_shells: vec![existing.clone()],
                    ..AppConfig::default()
                },
                runtime.handle(),
                writer,
            )
        })
    });
    let lists = subscribe_local_shells(cx, &state);
    let defaults = subscribe_default_shells(cx, &state);
    cx.executor().allow_parking();
    let choice = own_shell_choice("nu", "/bin/nu", "-l");

    let result = set_default_shell(cx, &state, choice.clone()).await;

    let changed = result.expect("the save reports the latest list and default");
    let expected = default_shell_updates(&choice);
    assert_eq!(changed.local_shells, vec![existing.clone()]);
    assert_eq!(changed.default_shell, expected);
    assert_eq!(*writes.lock().expect("default shell writes"), vec![choice]);
    assert_eq!(local_shells(cx, &state), vec![existing]);
    assert_eq!(default_shell(cx, &state), expected);
    assert_eq!(
        *lists.lock().expect("local shell events"),
        vec![changed.local_shells]
    );
    assert_eq!(
        *defaults.lock().expect("default shell events"),
        vec![changed.default_shell]
    );
}

#[gpui_kit::test]
async fn keeps_the_old_default_when_the_write_fails_and_remains_usable(cx: &mut TestAppContext) {
    let runtime = runtime();
    let existing = local_shell("existing", "/bin/zsh");
    let old = DefaultShellSettings {
        terminal_shell: "wsl".to_string(),
        terminal_shell_wsl_distro: "Debian".to_string(),
        ..DefaultShellSettings::default()
    };
    let fail = Arc::new(AtomicUsize::new(1));
    let writer_fail = Arc::clone(&fail);
    let shells = vec![existing.clone()];
    let writer_shells = shells.clone();
    let writer: DefaultShellWriter = Arc::new(move |choice| {
        if writer_fail.fetch_sub(1, Ordering::SeqCst) > 0 {
            return Err("disk is full".to_string());
        }
        Ok(SavedLocalShells {
            local_shells: writer_shells.clone(),
            default_shell: default_shell_updates(&choice),
        })
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_default_shell_writer(
                AppConfig {
                    terminal_shell: old.terminal_shell.clone(),
                    terminal_shell_wsl_distro: old.terminal_shell_wsl_distro.clone(),
                    local_shells: shells.clone(),
                    ..AppConfig::default()
                },
                runtime.handle(),
                writer,
            )
        })
    });
    let defaults = subscribe_default_shells(cx, &state);
    cx.executor().allow_parking();
    let reset = LocalShellChoice::default();

    let result = set_default_shell(cx, &state, reset.clone()).await;

    assert_eq!(result, Err("disk is full".to_string()));
    assert_eq!(default_shell(cx, &state), old);
    assert!(defaults.lock().expect("default shell events").is_empty());
    assert!(!save_pending(cx, &state));

    let result = set_default_shell(cx, &state, reset.clone()).await;
    assert!(result.is_ok());
    assert_eq!(default_shell(cx, &state), default_shell_updates(&reset));
    assert_eq!(local_shells(cx, &state), shells);
    assert_eq!(
        defaults.lock().expect("default shell events").len(),
        1,
        "only the successful save emits an event"
    );
}

#[gpui_kit::test]
async fn resets_the_default_without_dropping_the_saved_shells(cx: &mut TestAppContext) {
    let runtime = runtime();
    let existing = local_shell("nu", "/bin/nu");
    let writer_shells = vec![existing.clone()];
    let writer: DefaultShellWriter = Arc::new(move |choice| {
        Ok(SavedLocalShells {
            local_shells: writer_shells.clone(),
            default_shell: default_shell_updates(&choice),
        })
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_default_shell_writer(
                AppConfig {
                    terminal_shell: "custom".to_string(),
                    terminal_shell_custom_path: "/bin/nu".to_string(),
                    terminal_shell_custom_args: "-l".to_string(),
                    terminal_shell_id: "nu".to_string(),
                    local_shells: vec![existing.clone()],
                    ..AppConfig::default()
                },
                runtime.handle(),
                writer,
            )
        })
    });
    cx.executor().allow_parking();

    let result = set_default_shell(cx, &state, LocalShellChoice::default()).await;

    assert!(result.is_ok());
    assert_eq!(
        default_shell(cx, &state),
        default_shell_updates(&LocalShellChoice::default())
    );
    assert_eq!(local_shells(cx, &state), vec![existing]);
}

#[gpui_kit::test]
fn detected_shells_are_published_without_probing_during_render(cx: &mut TestAppContext) {
    let runtime = runtime();
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writer(AppConfig::default(), runtime.handle(), Arc::new(|_| Ok(())))
        })
    });
    assert!(cx.read(|app| state.read(app).detected_shells().is_empty()));
    let profile = TerminalShellProfile {
        shell: "pwsh".to_string(),
        label: "PowerShell".to_string(),
        source: "/usr/bin/pwsh".to_string(),
        ..TerminalShellProfile::default()
    };

    cx.update(|cx| {
        state.update(cx, |state, _| {
            state.set_detected_shells_for_test(vec![profile])
        })
    });

    cx.read(|app| {
        let detected = state.read(app).detected_shells();
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].shell, "pwsh");
        assert_eq!(detected[0].source, "/usr/bin/pwsh");
    });
}

fn terminal_font(cx: &TestAppContext, state: &Entity<ConfigState>) -> (String, u16) {
    cx.read(|app| {
        let config = state.read(app).config();
        (config.font_family.clone(), config.font_size)
    })
}

fn save_terminal_font(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
    family: &str,
    size: u16,
) -> Task<Result<(), String>> {
    let family = family.to_string();
    cx.update(|cx| state.update(cx, |state, cx| state.save_terminal_font(family, size, cx)))
}

fn subscribe_terminal_font(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
) -> Arc<Mutex<Vec<(String, u16)>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    cx.update(|cx| {
        cx.subscribe(state, move |_, event: &TerminalFontChanged, _| {
            recorded
                .lock()
                .expect("font events")
                .push((event.family.clone(), event.size));
        })
        .detach();
    });
    events
}

fn blocking_terminal_font_writer(
    entered: mpsc::Sender<()>,
    latch: WriterLatch,
) -> TerminalFontWriter {
    Arc::new(move |family, size| {
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
        Ok(SavedTerminalFont { family, size })
    })
}

#[gpui_kit::test]
async fn saves_the_terminal_font_then_publishes_the_saved_values(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let writer: TerminalFontWriter = Arc::new(move |family, size| {
        recorded
            .lock()
            .expect("font writes")
            .push((family.clone(), size));
        Ok(SavedTerminalFont {
            family: family.trim().to_string(),
            size,
        })
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_terminal_font_writer(AppConfig::default(), runtime.handle(), writer)
        })
    });
    let events = subscribe_terminal_font(cx, &state);
    cx.executor().allow_parking();

    let result = save_terminal_font(cx, &state, "  Menlo, monospace  ", 18).await;

    assert_eq!(result, Ok(()));
    assert_eq!(
        *writes.lock().expect("font writes"),
        vec![("Menlo, monospace".to_string(), 18)]
    );
    assert_eq!(
        terminal_font(cx, &state),
        ("Menlo, monospace".to_string(), 18)
    );
    assert_eq!(
        *events.lock().expect("font events"),
        vec![("Menlo, monospace".to_string(), 18)]
    );
}

#[gpui_kit::test]
async fn keeps_the_old_font_when_the_write_fails_and_remains_usable(cx: &mut TestAppContext) {
    let runtime = runtime();
    let fail = Arc::new(AtomicUsize::new(1));
    let writer_fail = Arc::clone(&fail);
    let writer: TerminalFontWriter = Arc::new(move |family, size| {
        if writer_fail.fetch_sub(1, Ordering::SeqCst) > 0 {
            Err("disk is full".to_string())
        } else {
            Ok(SavedTerminalFont { family, size })
        }
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_terminal_font_writer(
                AppConfig {
                    font_family: "Menlo".to_string(),
                    font_size: 14,
                    ..AppConfig::default()
                },
                runtime.handle(),
                writer,
            )
        })
    });
    let events = subscribe_terminal_font(cx, &state);
    cx.executor().allow_parking();

    let result = save_terminal_font(cx, &state, "Fira Code", 20).await;

    assert_eq!(result, Err("disk is full".to_string()));
    assert_eq!(terminal_font(cx, &state), ("Menlo".to_string(), 14));
    assert!(events.lock().expect("font events").is_empty());
    assert!(!save_pending(cx, &state));

    let result = save_terminal_font(cx, &state, "Fira Code", 20).await;
    assert_eq!(result, Ok(()));
    assert_eq!(terminal_font(cx, &state), ("Fira Code".to_string(), 20));
    assert_eq!(
        *events.lock().expect("font events"),
        vec![("Fira Code".to_string(), 20)]
    );
}

#[gpui_kit::test]
async fn an_unchanged_font_skips_the_write_and_the_event(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writes = Arc::new(AtomicUsize::new(0));
    let writer_writes = Arc::clone(&writes);
    let writer: TerminalFontWriter = Arc::new(move |family, size| {
        writer_writes.fetch_add(1, Ordering::SeqCst);
        Ok(SavedTerminalFont { family, size })
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_terminal_font_writer(
                AppConfig {
                    font_family: "Menlo".to_string(),
                    font_size: 14,
                    ..AppConfig::default()
                },
                runtime.handle(),
                writer,
            )
        })
    });
    let events = subscribe_terminal_font(cx, &state);
    cx.executor().allow_parking();

    let result = save_terminal_font(cx, &state, "Menlo", 14).await;

    assert_eq!(result, Ok(()));
    assert_eq!(writes.load(Ordering::SeqCst), 0);
    assert!(events.lock().expect("font events").is_empty());
}

#[gpui_kit::test]
async fn refuses_a_font_edit_while_another_save_is_pending(cx: &mut TestAppContext) {
    let runtime = runtime();
    let (entered_tx, entered_rx) = mpsc::channel();
    let latch: WriterLatch = Arc::new((Mutex::new(false), Condvar::new()));
    let release_on_drop = ReleaseOnDrop(Arc::clone(&latch));
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writer(
                AppConfig::default(),
                runtime.handle(),
                blocking_writer(entered_tx, Arc::clone(&latch)),
            )
        })
    });
    let events = subscribe_terminal_font(cx, &state);
    cx.executor().allow_parking();

    let scrollback = save(cx, &state, 20_000);
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the scrollback writer entered its blocking section");
    let result = save_terminal_font(cx, &state, "Menlo", 16).await;

    assert!(
        result.is_err(),
        "a font edit cannot start from a stale snapshot"
    );
    assert_eq!(
        terminal_font(cx, &state),
        (AppConfig::default().font_family, 14)
    );
    assert!(events.lock().expect("font events").is_empty());

    release(&latch);
    assert_eq!(scrollback.await, Ok(20_000));
    drop(release_on_drop);
}

#[gpui_kit::test]
async fn a_dropped_font_waiter_still_commits_and_releases_pending(cx: &mut TestAppContext) {
    let runtime = runtime();
    let (entered_tx, entered_rx) = mpsc::channel();
    let latch: WriterLatch = Arc::new((Mutex::new(false), Condvar::new()));
    let release_on_drop = ReleaseOnDrop(Arc::clone(&latch));
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_terminal_font_writer(
                AppConfig::default(),
                runtime.handle(),
                blocking_terminal_font_writer(entered_tx, Arc::clone(&latch)),
            )
        })
    });
    let events = subscribe_terminal_font(cx, &state);
    cx.executor().allow_parking();

    let first = save_terminal_font(cx, &state, "Menlo", 16);
    drop(first);
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the font writer entered its blocking section");

    release(&latch);
    wait_until(cx, |cx| !save_pending(cx, &state)).await;

    assert_eq!(terminal_font(cx, &state), ("Menlo".to_string(), 16));
    assert_eq!(
        *events.lock().expect("font events"),
        vec![("Menlo".to_string(), 16)]
    );

    let second = save_terminal_font(cx, &state, "Fira Code", 17).await;
    assert_eq!(second, Ok(()));
    assert_eq!(terminal_font(cx, &state), ("Fira Code".to_string(), 17));

    drop(release_on_drop);
}

#[gpui_kit::test]
async fn edits_the_isolated_shell_lists_and_reloads_the_latest_snapshot(cx: &mut TestAppContext) {
    let runtime = runtime();
    let zsh = local_shell("zsh", "/bin/zsh");
    let nu = local_shell("nu", "/bin/nu");
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writer(
                AppConfig {
                    local_shells: vec![zsh.clone(), nu.clone()],
                    hidden_shells: vec!["cmd".to_string()],
                    ..AppConfig::default()
                },
                runtime.handle(),
                Arc::new(|_| Ok(())),
            )
        })
    });
    let events = subscribe_shell_lists(cx, &state);
    cx.executor().allow_parking();

    let moved = edit_shell_list(
        cx,
        &state,
        ShellListEdit::Move {
            id: "nu".to_string(),
            offset: -1,
        },
    )
    .await
    .expect("the move reports the list");
    assert_eq!(moved.local_shells, vec![nu.clone(), zsh.clone()]);
    assert_eq!(moved.hidden_shells, ["cmd"]);

    let hidden = edit_shell_list(
        cx,
        &state,
        ShellListEdit::Visibility {
            key: "pwsh".to_string(),
            hidden: true,
        },
    )
    .await
    .expect("the hide reports the list");
    assert_eq!(hidden.hidden_shells, ["cmd", "pwsh"]);
    assert_eq!(
        hidden.local_shells,
        vec![nu.clone(), zsh.clone()],
        "the writer reloads the earlier edit instead of the startup list"
    );

    let again = edit_shell_list(
        cx,
        &state,
        ShellListEdit::Visibility {
            key: "pwsh".to_string(),
            hidden: true,
        },
    )
    .await
    .expect("hiding twice is allowed");
    assert_eq!(again.hidden_shells, ["cmd", "pwsh"]);

    let shown = edit_shell_list(
        cx,
        &state,
        ShellListEdit::Visibility {
            key: "cmd".to_string(),
            hidden: false,
        },
    )
    .await
    .expect("the show reports the list");
    assert_eq!(shown.hidden_shells, ["pwsh"]);
    assert_eq!(local_shells(cx, &state), vec![nu, zsh]);
    assert_eq!(hidden_shells(cx, &state), ["pwsh"]);
    assert!(!save_pending(cx, &state));

    let events = events.lock().expect("shell list events");
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].0, moved.local_shells);
    assert_eq!(events[1].1, ["cmd", "pwsh"]);
    assert_eq!(events[3].1, ["pwsh"]);
    assert_eq!(events[3].2, default_shell(cx, &state));
}

#[gpui_kit::test]
async fn refuses_list_edits_the_lists_cannot_take(cx: &mut TestAppContext) {
    let runtime = runtime();
    let nu = local_shell("nu", "/bin/nu");
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writer(
                AppConfig {
                    terminal_shell: "pwsh".to_string(),
                    local_shells: vec![nu.clone()],
                    ..AppConfig::default()
                },
                runtime.handle(),
                Arc::new(|_| Ok(())),
            )
        })
    });
    let events = subscribe_shell_lists(cx, &state);
    cx.executor().allow_parking();

    let rejected = [
        ShellListEdit::Visibility {
            key: "auto".to_string(),
            hidden: true,
        },
        ShellListEdit::Visibility {
            key: "custom:nu".to_string(),
            hidden: true,
        },
        ShellListEdit::Visibility {
            key: "pwsh".to_string(),
            hidden: true,
        },
        ShellListEdit::Move {
            id: "nu".to_string(),
            offset: 0,
        },
        ShellListEdit::Move {
            id: "nu".to_string(),
            offset: 2,
        },
    ];
    for edit in rejected {
        assert!(edit_shell_list(cx, &state, edit).await.is_err());
    }

    assert!(events.lock().expect("shell list events").is_empty());
    assert_eq!(local_shells(cx, &state), vec![nu]);
    assert!(hidden_shells(cx, &state).is_empty());
    assert!(!save_pending(cx, &state));
}

#[gpui_kit::test]
async fn removes_the_default_shell_and_falls_back_to_auto(cx: &mut TestAppContext) {
    let runtime = runtime();
    let nu = local_shell("nu", "/bin/nu");
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writer(
                AppConfig {
                    terminal_shell: "custom".to_string(),
                    terminal_shell_custom_path: "/bin/nu".to_string(),
                    terminal_shell_custom_args: "-l".to_string(),
                    terminal_shell_id: "nu".to_string(),
                    hidden_shells: vec!["cmd".to_string()],
                    local_shells: vec![nu],
                    ..AppConfig::default()
                },
                runtime.handle(),
                Arc::new(|_| Ok(())),
            )
        })
    });
    let events = subscribe_shell_lists(cx, &state);
    cx.executor().allow_parking();

    let changed = edit_shell_list(
        cx,
        &state,
        ShellListEdit::Remove {
            id: "nu".to_string(),
        },
    )
    .await
    .expect("the removal reports the list");

    assert!(changed.local_shells.is_empty());
    assert_eq!(
        changed.default_shell,
        default_shell_updates(&LocalShellChoice::default())
    );
    assert_eq!(
        changed.hidden_shells,
        ["cmd"],
        "the hidden list survives a removal"
    );
    assert!(local_shells(cx, &state).is_empty());
    assert_eq!(default_shell(cx, &state), changed.default_shell);
    let events = events.lock().expect("shell list events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, changed.local_shells);
    assert_eq!(events[0].1, ["cmd"]);
}

#[gpui_kit::test]
async fn keeps_the_lists_when_the_list_write_fails_and_remains_usable(cx: &mut TestAppContext) {
    let runtime = runtime();
    let nu = local_shell("nu", "/bin/nu");
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_writer(
                AppConfig {
                    local_shells: vec![nu.clone()],
                    hidden_shells: vec!["cmd".to_string()],
                    ..AppConfig::default()
                },
                runtime.handle(),
                Arc::new(|_| Ok(())),
            )
        })
    });
    let fail = Arc::new(AtomicUsize::new(1));
    let writer_shells = vec![nu.clone()];
    let writer: ShellListWriter = Arc::new(move |_| {
        if fail.fetch_sub(1, Ordering::SeqCst) > 0 {
            return Err("disk is full".to_string());
        }
        Ok(SavedShellLists {
            local_shells: writer_shells.clone(),
            hidden_shells: vec!["pwsh".to_string()],
            default_shell: default_shell_updates(&LocalShellChoice::default()),
        })
    });
    cx.update(|cx| state.update(cx, |state, _| state.set_shell_list_writer_for_test(writer)));
    let events = subscribe_shell_lists(cx, &state);
    cx.executor().allow_parking();
    let hide = ShellListEdit::Visibility {
        key: "pwsh".to_string(),
        hidden: true,
    };

    let result = edit_shell_list(cx, &state, hide.clone()).await;

    assert_eq!(result, Err("disk is full".to_string()));
    assert_eq!(hidden_shells(cx, &state), ["cmd"]);
    assert!(events.lock().expect("shell list events").is_empty());
    assert!(!save_pending(cx, &state));

    let result = edit_shell_list(cx, &state, hide).await;
    assert!(result.is_ok());
    assert_eq!(hidden_shells(cx, &state), ["pwsh"]);
    assert_eq!(local_shells(cx, &state), vec![nu]);
    assert_eq!(
        events.lock().expect("shell list events").len(),
        1,
        "only the successful edit emits an event"
    );
}

async fn wait_until(cx: &TestAppContext, mut condition: impl FnMut(&TestAppContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition(cx) {
        assert!(Instant::now() < deadline, "timed out waiting for the save");
        cx.executor().timer(Duration::from_millis(10)).await;
    }
}
