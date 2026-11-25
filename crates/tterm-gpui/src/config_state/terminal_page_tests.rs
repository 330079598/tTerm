//! Terminal page save tests: one commit, the events it emits and its refusals.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::Duration;

use gpui_kit::prelude::*;
use gpui_kit::{Entity, Task, TestAppContext};
use tterm_core::config::AppConfig;
use tterm_core::config::terminal_font_file::{SavedTerminalFont, TerminalPageSettings};

use super::{
    ConfigState, ScrollbackChanged, ScrollbackWriter, TerminalFontChanged, TerminalFontWriter,
    TerminalPageWriter,
};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime")
}

fn page(cx: &TestAppContext, state: &Entity<ConfigState>) -> TerminalPageSettings {
    cx.read(|app| TerminalPageSettings::from(state.read(app).config()))
}

fn save(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
    settings: TerminalPageSettings,
) -> Task<Result<(), String>> {
    cx.update(|cx| state.update(cx, |state, cx| state.save_terminal_page(settings, cx)))
}

fn pending(cx: &TestAppContext, state: &Entity<ConfigState>) -> bool {
    cx.read(|app| state.read(app).is_save_pending())
}

type FontEvents = Arc<Mutex<Vec<TerminalFontChanged>>>;

fn subscribe_font(cx: &mut TestAppContext, state: &Entity<ConfigState>) -> FontEvents {
    let events: FontEvents = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    cx.update(|cx| {
        cx.subscribe(state, move |_, event: &TerminalFontChanged, _| {
            recorded.lock().expect("font events").push(event.clone());
        })
        .detach();
    });
    events
}

fn subscribe_scrollback(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
) -> Arc<Mutex<Vec<u32>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    cx.update(|cx| {
        cx.subscribe(state, move |_, event: &ScrollbackChanged, _| {
            recorded
                .lock()
                .expect("scrollback events")
                .push(event.lines);
        })
        .detach();
    });
    events
}

#[gpui_kit::test]
async fn saves_the_page_then_publishes_the_actual_values_and_both_events(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let writer: TerminalPageWriter = Arc::new(move |settings| {
        recorded.lock().expect("page writes").push(settings.clone());
        // The file keeps only one decimal of the line height.
        Ok(TerminalPageSettings {
            line_height: (settings.line_height * 10.0).round() / 10.0,
            ..settings
        })
    });
    let start = AppConfig {
        font_family: "Menlo".to_string(),
        font_size: 14,
        terminal_line_height: 1.0,
        terminal_letter_spacing: 0,
        scrollback_lines: 10_000,
        ..AppConfig::default()
    };
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_terminal_page_writer(start, runtime.handle(), writer))
    });
    let font_events = subscribe_font(cx, &state);
    let scrollback_events = subscribe_scrollback(cx, &state);
    cx.executor().allow_parking();

    let result = save(
        cx,
        &state,
        TerminalPageSettings {
            family: "  Fira Code  ".to_string(),
            size: 20,
            line_height: 1.26,
            letter_spacing: 2,
            scrollback_lines: 50_000,
            cursor_style: "underline".to_string(),
            padding_left: 12,
            padding_right: 14,
            padding_bottom: 8,
            confirm_multiline_paste: false,
            copy_on_select: true,
            right_click_paste: true,
            osc52_clipboard_write: false,
            mac_option_is_meta: true,
        },
    )
    .await;

    assert_eq!(result, Ok(()));
    assert_eq!(
        *writes.lock().expect("page writes"),
        vec![TerminalPageSettings {
            family: "Fira Code".to_string(),
            size: 20,
            line_height: 1.26,
            letter_spacing: 2,
            scrollback_lines: 50_000,
            cursor_style: "underline".to_string(),
            padding_left: 12,
            padding_right: 14,
            padding_bottom: 8,
            confirm_multiline_paste: false,
            copy_on_select: true,
            right_click_paste: true,
            osc52_clipboard_write: false,
            mac_option_is_meta: true,
        }],
        "the checked draft reaches the one page writer"
    );
    assert_eq!(
        page(cx, &state),
        TerminalPageSettings {
            family: "Fira Code".to_string(),
            size: 20,
            line_height: 1.3,
            letter_spacing: 2,
            scrollback_lines: 50_000,
            cursor_style: "underline".to_string(),
            padding_left: 12,
            padding_right: 14,
            padding_bottom: 8,
            confirm_multiline_paste: false,
            copy_on_select: true,
            right_click_paste: true,
            osc52_clipboard_write: false,
            mac_option_is_meta: true,
        },
        "the snapshot publishes what the writer reported, not the draft"
    );
    assert_eq!(
        *font_events.lock().expect("font events"),
        vec![TerminalFontChanged {
            family: "Fira Code".to_string(),
            size: 20,
            line_height: 1.3,
            letter_spacing: 2,
            confirm_multiline_paste: false,
            copy_on_select: true,
            right_click_paste: true,
        }]
    );
    assert_eq!(
        *scrollback_events.lock().expect("scrollback events"),
        vec![50_000]
    );
}

#[gpui_kit::test]
async fn publishes_the_clipboard_preferences_the_file_reported(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writer: TerminalPageWriter = Arc::new(|mut settings| {
        // The save reads the page back, so values the file kept from an
        // external edit win over the draft this window submitted.
        settings.confirm_multiline_paste = false;
        settings.copy_on_select = true;
        settings.right_click_paste = true;
        Ok(settings)
    });
    let start = AppConfig {
        confirm_multiline_paste: true,
        copy_on_select: false,
        right_click_paste: false,
        ..AppConfig::default()
    };
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_terminal_page_writer(start, runtime.handle(), writer))
    });
    let font_events = subscribe_font(cx, &state);
    cx.executor().allow_parking();

    let mut draft = page(cx, &state);
    draft.size += 1;
    let result = save(cx, &state, draft).await;

    assert_eq!(result, Ok(()));
    let saved = page(cx, &state);
    assert_eq!(
        (
            saved.confirm_multiline_paste,
            saved.copy_on_select,
            saved.right_click_paste,
        ),
        (false, true, true),
        "the snapshot publishes the readback, not the draft"
    );
    assert_eq!(
        *font_events.lock().expect("font events"),
        vec![TerminalFontChanged {
            family: saved.family,
            size: saved.size,
            line_height: saved.line_height,
            letter_spacing: saved.letter_spacing,
            confirm_multiline_paste: false,
            copy_on_select: true,
            right_click_paste: true,
        }]
    );
}

#[gpui_kit::test]
async fn publishes_the_osc52_and_option_values_the_file_reported(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writer: TerminalPageWriter = Arc::new(|mut settings| {
        // Another client changed the two preferences this window kept, so the
        // readback wins over the draft the page submitted.
        settings.osc52_clipboard_write = false;
        settings.mac_option_is_meta = true;
        Ok(settings)
    });
    let start = AppConfig {
        osc52_clipboard_write: true,
        mac_option_is_meta: false,
        ..AppConfig::default()
    };
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_terminal_page_writer(start, runtime.handle(), writer))
    });
    let font_events = subscribe_font(cx, &state);
    cx.executor().allow_parking();

    let mut draft = page(cx, &state);
    draft.size += 1;
    let result = save(cx, &state, draft).await;

    assert_eq!(result, Ok(()));
    let saved = page(cx, &state);
    assert_eq!(
        (saved.osc52_clipboard_write, saved.mac_option_is_meta),
        (false, true),
        "the snapshot publishes the readback, not the draft"
    );
    assert_eq!(
        *font_events.lock().expect("font events"),
        vec![TerminalFontChanged {
            family: saved.family,
            size: saved.size,
            line_height: saved.line_height,
            letter_spacing: saved.letter_spacing,
            confirm_multiline_paste: saved.confirm_multiline_paste,
            copy_on_select: saved.copy_on_select,
            right_click_paste: saved.right_click_paste,
        }],
        "the event keeps its fields; the runtime reads the new snapshot values"
    );
}

#[gpui_kit::test]
async fn a_font_only_page_does_not_emit_a_scrollback_change(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writer: TerminalPageWriter = Arc::new(Ok);
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_terminal_page_writer(AppConfig::default(), runtime.handle(), writer)
        })
    });
    let font_events = subscribe_font(cx, &state);
    let scrollback_events = subscribe_scrollback(cx, &state);
    cx.executor().allow_parking();

    let mut draft = page(cx, &state);
    draft.family = "Fira Code, monospace".to_string();
    draft.size = 18;
    let result = save(cx, &state, draft.clone()).await;

    assert_eq!(result, Ok(()));
    assert_eq!(page(cx, &state), draft);
    assert_eq!(
        *font_events.lock().expect("font events"),
        vec![TerminalFontChanged {
            family: "Fira Code, monospace".to_string(),
            size: 18,
            line_height: draft.line_height,
            letter_spacing: draft.letter_spacing,
            confirm_multiline_paste: draft.confirm_multiline_paste,
            copy_on_select: draft.copy_on_select,
            right_click_paste: draft.right_click_paste,
        }]
    );
    assert!(
        scrollback_events
            .lock()
            .expect("scrollback events")
            .is_empty(),
        "an unchanged capacity must not move the scrollback buffer"
    );
}

#[gpui_kit::test]
async fn a_cursor_and_padding_only_page_updates_the_snapshot_before_the_font_event(
    cx: &mut TestAppContext,
) {
    let runtime = runtime();
    let writer: TerminalPageWriter = Arc::new(Ok);
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_terminal_page_writer(AppConfig::default(), runtime.handle(), writer)
        })
    });
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    cx.update(|cx| {
        cx.subscribe(&state, move |state, event: &TerminalFontChanged, cx| {
            let config = state.read(cx).config();
            recorded.lock().expect("page snapshot at event").push((
                event.family.clone(),
                config.cursor_style.clone(),
                config.terminal_padding_left_px,
                config.terminal_padding_right_px,
                config.terminal_padding_bottom_px,
                config.confirm_multiline_paste,
                config.copy_on_select,
                config.right_click_paste,
                config.osc52_clipboard_write,
                config.mac_option_is_meta,
            ));
        })
        .detach();
    });
    let font_events = subscribe_font(cx, &state);
    let scrollback_events = subscribe_scrollback(cx, &state);
    cx.executor().allow_parking();

    let mut draft = page(cx, &state);
    draft.cursor_style = "bar".to_string();
    draft.padding_left = 12;
    draft.padding_right = 14;
    draft.padding_bottom = 8;
    draft.confirm_multiline_paste = false;
    draft.copy_on_select = true;
    draft.right_click_paste = true;
    draft.osc52_clipboard_write = false;
    draft.mac_option_is_meta = true;
    let result = save(cx, &state, draft.clone()).await;

    assert_eq!(result, Ok(()));
    assert_eq!(page(cx, &state), draft);
    assert_eq!(
        *seen.lock().expect("page snapshot at event"),
        vec![(
            draft.family.clone(),
            "bar".to_string(),
            12,
            14,
            8,
            false,
            true,
            true,
            false,
            true,
        )],
        "the listener sees the cursor, padding, clipboard and terminal behaviour values the event follows"
    );
    assert_eq!(
        *font_events.lock().expect("font events"),
        vec![TerminalFontChanged {
            family: draft.family.clone(),
            size: draft.size,
            line_height: draft.line_height,
            letter_spacing: draft.letter_spacing,
            confirm_multiline_paste: false,
            copy_on_select: true,
            right_click_paste: true,
        }]
    );
    assert!(
        scrollback_events
            .lock()
            .expect("scrollback events")
            .is_empty(),
        "a cursor and padding edit must not move the scrollback buffer"
    );
}

#[gpui_kit::test]
async fn keeps_the_old_page_when_the_write_fails_and_remains_usable(cx: &mut TestAppContext) {
    let runtime = runtime();
    let fail = Arc::new(AtomicUsize::new(1));
    let writer_fail = Arc::clone(&fail);
    let writer: TerminalPageWriter = Arc::new(move |settings| {
        if writer_fail.fetch_sub(1, Ordering::SeqCst) > 0 {
            Err("disk is full".to_string())
        } else {
            Ok(settings)
        }
    });
    let start = AppConfig {
        font_family: "Menlo".to_string(),
        font_size: 14,
        scrollback_lines: 10_000,
        osc52_clipboard_write: true,
        mac_option_is_meta: false,
        ..AppConfig::default()
    };
    let before = TerminalPageSettings::from(&start);
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_terminal_page_writer(start.clone(), runtime.handle(), writer))
    });
    let font_events = subscribe_font(cx, &state);
    let scrollback_events = subscribe_scrollback(cx, &state);
    cx.executor().allow_parking();

    let mut draft = before.clone();
    draft.family = "Fira Code".to_string();
    draft.size = 20;
    draft.scrollback_lines = 50_000;
    draft.confirm_multiline_paste = true;
    draft.copy_on_select = true;
    draft.right_click_paste = true;
    draft.osc52_clipboard_write = false;
    draft.mac_option_is_meta = true;
    let result = save(cx, &state, draft.clone()).await;

    assert_eq!(result, Err("disk is full".to_string()));
    assert_eq!(page(cx, &state), before, "a failed write commits nothing");
    assert_eq!(
        (
            page(cx, &state).osc52_clipboard_write,
            page(cx, &state).mac_option_is_meta,
        ),
        (true, false),
        "a failed write leaves the old OSC 52 and Option policy in place"
    );
    assert!(font_events.lock().expect("font events").is_empty());
    assert!(
        scrollback_events
            .lock()
            .expect("scrollback events")
            .is_empty()
    );
    assert!(!pending(cx, &state));

    assert_eq!(save(cx, &state, draft.clone()).await, Ok(()));
    assert_eq!(page(cx, &state), draft);
    assert_eq!(
        (
            page(cx, &state).osc52_clipboard_write,
            page(cx, &state).mac_option_is_meta,
        ),
        (false, true),
        "a retried edit publishes the readback after the write succeeds"
    );
    assert_eq!(
        *font_events.lock().expect("font events"),
        vec![TerminalFontChanged {
            family: "Fira Code".to_string(),
            size: 20,
            line_height: before.line_height,
            letter_spacing: before.letter_spacing,
            confirm_multiline_paste: true,
            copy_on_select: true,
            right_click_paste: true,
        }],
        "a retried clipboard edit publishes only after the write succeeds"
    );
    assert_eq!(
        *scrollback_events.lock().expect("scrollback events"),
        vec![50_000]
    );
}

#[gpui_kit::test]
async fn refuses_an_invalid_page_before_the_writer_runs(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writes = Arc::new(AtomicUsize::new(0));
    let writer_writes = Arc::clone(&writes);
    let writer: TerminalPageWriter = Arc::new(move |settings| {
        writer_writes.fetch_add(1, Ordering::SeqCst);
        Ok(settings)
    });
    let start = AppConfig::default();
    let before = TerminalPageSettings::from(&start);
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_terminal_page_writer(start, runtime.handle(), writer))
    });
    let font_events = subscribe_font(cx, &state);
    cx.executor().allow_parking();

    for draft in [
        TerminalPageSettings {
            family: String::new(),
            ..before.clone()
        },
        TerminalPageSettings {
            family: " \t ".to_string(),
            ..before.clone()
        },
        TerminalPageSettings {
            family: "Menlo\nmonospace".to_string(),
            ..before.clone()
        },
        TerminalPageSettings {
            size: 5,
            ..before.clone()
        },
        TerminalPageSettings {
            size: 73,
            ..before.clone()
        },
        TerminalPageSettings {
            line_height: f64::NAN,
            ..before.clone()
        },
        TerminalPageSettings {
            line_height: 2.5,
            ..before.clone()
        },
        TerminalPageSettings {
            letter_spacing: -6,
            ..before.clone()
        },
        TerminalPageSettings {
            letter_spacing: 11,
            ..before.clone()
        },
        TerminalPageSettings {
            cursor_style: "beam".to_string(),
            ..before.clone()
        },
        TerminalPageSettings {
            padding_left: 81,
            ..before.clone()
        },
        TerminalPageSettings {
            padding_right: 81,
            ..before.clone()
        },
        TerminalPageSettings {
            padding_bottom: 81,
            ..before.clone()
        },
    ] {
        assert!(
            save(cx, &state, draft.clone()).await.is_err(),
            "{draft:?} must be refused"
        );
        assert_eq!(page(cx, &state), before);
        assert!(!pending(cx, &state));
    }

    assert_eq!(writes.load(Ordering::SeqCst), 0);
    assert!(font_events.lock().expect("font events").is_empty());
}

#[gpui_kit::test]
async fn an_unchanged_page_skips_the_write_and_the_events(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writes = Arc::new(AtomicUsize::new(0));
    let writer_writes = Arc::clone(&writes);
    let writer: TerminalPageWriter = Arc::new(move |settings| {
        writer_writes.fetch_add(1, Ordering::SeqCst);
        Ok(settings)
    });
    let state = cx.update(|cx| {
        cx.new(|_| {
            ConfigState::with_terminal_page_writer(AppConfig::default(), runtime.handle(), writer)
        })
    });
    let font_events = subscribe_font(cx, &state);
    let scrollback_events = subscribe_scrollback(cx, &state);
    cx.executor().allow_parking();

    let result = save(cx, &state, page(cx, &state)).await;

    assert_eq!(result, Ok(()));
    assert_eq!(writes.load(Ordering::SeqCst), 0);
    assert!(font_events.lock().expect("font events").is_empty());
    assert!(
        scrollback_events
            .lock()
            .expect("scrollback events")
            .is_empty()
    );
}

type WriterLatch = Arc<(Mutex<bool>, Condvar)>;

/// Releases a blocked writer when it drops, so a failed assertion cannot hang
/// the Tokio runtime while it waits for blocking tasks to finish.
struct ReleaseOnDrop(WriterLatch);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        let (lock, condvar) = &*self.0;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condvar.notify_all();
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
async fn refuses_a_page_edit_while_another_save_is_pending(cx: &mut TestAppContext) {
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
    let font_events = subscribe_font(cx, &state);
    cx.executor().allow_parking();

    let scrollback =
        cx.update(|cx| state.update(cx, |state, cx| state.save_scrollback_lines(20_000, cx)));
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the scrollback writer entered its blocking section");
    let mut draft = page(cx, &state);
    draft.family = "Fira Code".to_string();
    let result = save(cx, &state, draft).await;

    assert!(
        result.is_err(),
        "a page edit cannot start from a stale snapshot"
    );
    assert_eq!(
        page(cx, &state),
        TerminalPageSettings::from(&AppConfig::default())
    );
    assert!(font_events.lock().expect("font events").is_empty());

    release(&latch);
    assert_eq!(scrollback.await, Ok(20_000));
    drop(release_on_drop);
}

fn save_font(
    cx: &mut TestAppContext,
    state: &Entity<ConfigState>,
    family: &str,
    size: u16,
) -> Task<Result<(), String>> {
    let family = family.to_string();
    cx.update(|cx| state.update(cx, |state, cx| state.save_terminal_font(family, size, cx)))
}

#[gpui_kit::test]
async fn the_old_font_save_reports_the_pages_line_settings(cx: &mut TestAppContext) {
    let runtime = runtime();
    let writer: TerminalFontWriter =
        Arc::new(|family, size| Ok(SavedTerminalFont { family, size }));
    let start = AppConfig {
        font_family: "Menlo".to_string(),
        font_size: 14,
        terminal_line_height: 1.4,
        terminal_letter_spacing: 2,
        scrollback_lines: 10_000,
        confirm_multiline_paste: false,
        copy_on_select: true,
        right_click_paste: false,
        ..AppConfig::default()
    };
    let state = cx.update(|cx| {
        cx.new(|_| ConfigState::with_terminal_font_writer(start, runtime.handle(), writer))
    });
    let font_events = subscribe_font(cx, &state);
    cx.executor().allow_parking();

    assert_eq!(save_font(cx, &state, "Fira Code", 20).await, Ok(()));

    assert_eq!(
        *font_events.lock().expect("font events"),
        vec![TerminalFontChanged {
            family: "Fira Code".to_string(),
            size: 20,
            line_height: 1.4,
            letter_spacing: 2,
            confirm_multiline_paste: false,
            copy_on_select: true,
            right_click_paste: false,
        }]
    );
    let saved = page(cx, &state);
    assert_eq!((saved.line_height, saved.letter_spacing), (1.4, 2));
    assert_eq!(
        (
            saved.confirm_multiline_paste,
            saved.copy_on_select,
            saved.right_click_paste,
        ),
        (false, true, false)
    );
    assert_eq!(saved.scrollback_lines, 10_000);
}
