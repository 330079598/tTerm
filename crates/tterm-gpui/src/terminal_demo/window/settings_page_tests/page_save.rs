//! Route tests for the one saved draft below the Terminal page.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, mpsc};

use gpui_kit::InputEvent as _;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{ScrollDelta, ScrollEvent};

use super::*;

pub(super) fn echo_page_writer(
    writes: Arc<Mutex<Vec<TerminalPageSettings>>>,
) -> TerminalPageWriter {
    Arc::new(move |settings| {
        writes.lock().expect("page writes").push(settings.clone());
        Ok(settings)
    })
}

pub(super) fn wait_for_page_writes(
    writes: &Arc<Mutex<Vec<TerminalPageSettings>>>,
    expected: usize,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if writes.lock().expect("page writes").len() >= expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected} page write(s)"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn wait_for_page_idle(harness: &mut Harness<'_>, root: &Entity<Prototype>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let idle = harness.visual.update(|_, cx| {
            let state = root.read(cx).ui.settings_page.read(cx);
            !state.page_saving() && !harness.config.read(cx).is_save_pending()
        });
        if idle {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the page save"
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn open_font_draft(harness: &mut Harness<'_>, root: &Entity<Prototype>) {
    let page = harness
        .visual
        .update(|_, cx| root.read(cx).ui.settings_page.clone());
    harness.visual.update(|_, cx| {
        page.update(cx, |page, cx| page.set_font_system_fonts(Vec::new(), cx));
    });
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(harness, "font family");
    press(harness.visual, "enter");
    replace_focused_text(harness, "Menlo, monospace");
    click_id(harness.visual, crate::font_setting::SIZE_INPUT_ID);
    replace_focused_text(harness, "20");
    click_id(harness.visual, crate::font_setting::LINE_HEIGHT_INPUT_ID);
    replace_focused_text(harness, "1.5");
    click_id(harness.visual, crate::font_setting::LETTER_SPACING_INPUT_ID);
    replace_focused_text(harness, "3");
    open_scrollback_from_search(harness, root);
    replace_focused_text(harness, "4321");
}

#[gpui_kit::test]
fn enter_saves_every_field_and_each_field_reruns_the_whole_page(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut harness = harness_with_page_writer(
        cx,
        AppConfig::default(),
        echo_page_writer(Arc::clone(&writes)),
    );
    let root = harness.root.clone();
    let config = harness.config.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2000.0)));
    open_font_draft(&mut harness, &root);

    // Enter in the scrollback field writes the whole page, not only the
    // scrollback capacity.
    press(harness.visual, "enter");
    wait_for_page_writes(&writes, 1);
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.page_saved());
        assert!(!page.page_saving());
        assert!(page.page_error().is_none());
        assert!(page.font_error(cx).is_none());
        let config = config.read(cx).config();
        assert_eq!(
            (
                config.font_family.as_str(),
                config.font_size,
                config.terminal_line_height,
                config.terminal_letter_spacing,
                config.scrollback_lines,
            ),
            ("Menlo, monospace", 20, 1.5, 3, 4321)
        );
    });
    assert_eq!(
        *writes.lock().expect("page writes"),
        vec![TerminalPageSettings {
            family: "Menlo, monospace".to_string(),
            size: 20,
            line_height: 1.5,
            letter_spacing: 3,
            scrollback_lines: 4321,
            ..TerminalPageSettings::from(&AppConfig::default())
        }]
    );

    // Enter in the font size field repeats the same complete draft; the
    // unchanged fields are not lost just because another input had focus.
    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(&mut harness, "font size");
    press(harness.visual, "enter");
    replace_focused_text(&mut harness, "22");
    press(harness.visual, "enter");
    wait_for_page_writes(&writes, 2);
    wait_for_page_idle(&mut harness, &root);
    let writes = writes.lock().expect("page writes");
    assert_eq!(writes.len(), 2);
    assert_eq!(
        writes[1],
        TerminalPageSettings {
            family: "Menlo, monospace".to_string(),
            size: 22,
            line_height: 1.5,
            letter_spacing: 3,
            scrollback_lines: 4321,
            ..TerminalPageSettings::from(&AppConfig::default())
        }
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_pending_save_blocks_a_second_submit_and_the_route_close(cx: &mut TestAppContext) {
    let attempts = Arc::new(AtomicUsize::new(0));
    let latch = Arc::new((Mutex::new(false), Condvar::new()));
    let (entered_tx, entered_rx) = mpsc::channel();
    let recorded = Arc::clone(&attempts);
    let writer_latch = Arc::clone(&latch);
    let writer: TerminalPageWriter = Arc::new(move |settings| {
        recorded.fetch_add(1, Ordering::SeqCst);
        let _ = entered_tx.send(());
        let (lock, condvar) = &*writer_latch;
        let mut released = lock
            .lock()
            .map_err(|_| "the latch was poisoned".to_string())?;
        while !*released {
            released = condvar
                .wait(released)
                .map_err(|_| "the latch was poisoned".to_string())?;
        }
        Ok(settings)
    });
    let mut harness = harness_with_page_writer(cx, AppConfig::default(), writer);
    let root = harness.root.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2000.0)));
    open_font_draft(&mut harness, &root);

    press(harness.visual, "enter");
    entered_rx
        .recv_timeout(Duration::from_secs(20))
        .expect("the page writer should start");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.page_saving());
        assert!(!page.page_saved());
        assert!(
            root.read(cx).settings_is_open(),
            "the route must stay open while the page save is pending"
        );
    });
    assert!(root.read_with(harness.visual, |workspace, _| workspace.settings_is_open()));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);

    // A second Enter and the close entry both hit the gate while the first
    // save owns the configuration entity.
    press(harness.visual, "enter");
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    let closed = harness
        .visual
        .update(|window, cx| root.update(cx, |workspace, cx| workspace.close_settings(window, cx)));
    assert!(!closed, "closing must wait for the pending page save");
    assert!(settings_page_visible(harness.visual));

    {
        let (lock, condvar) = &*latch;
        *lock.lock().expect("the latch") = true;
        condvar.notify_all();
    }
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(!root.read(cx).ui.settings_page.read(cx).page_saving());
        assert_eq!(harness.config.read(cx).scrollback_lines(), 4321);
    });
    let closed = harness
        .visual
        .update(|window, cx| root.update(cx, |workspace, cx| workspace.close_settings(window, cx)));
    assert!(closed);
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn invalid_font_or_scrollback_keeps_every_draft_and_writes_nothing(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut harness = harness_with_page_writer(
        cx,
        AppConfig::default(),
        echo_page_writer(Arc::clone(&writes)),
    );
    let root = harness.root.clone();
    let config = harness.config.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2000.0)));
    open_font_draft(&mut harness, &root);

    // A line height outside the shared range blocks the entire page even
    // though the other fields are valid.
    click_id(harness.visual, crate::font_setting::LINE_HEIGHT_INPUT_ID);
    replace_focused_text(&mut harness, "0.5");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(!page.page_saving());
        assert!(page.font_error(cx).is_some());
        assert_eq!(page.font_values(cx).0, "Menlo, monospace");
        assert_eq!(page.font_values(cx).1, "20");
        assert_eq!(
            page.font_spacing_values(cx),
            ("0.5".to_string(), "3".to_string())
        );
        assert_eq!(page.scrollback_value(cx), "4321");
        assert!(page_font_line_height_focused(&root, window, cx));
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
    });
    assert!(writes.lock().expect("page writes").is_empty());

    click_id(harness.visual, crate::font_setting::LINE_HEIGHT_INPUT_ID);
    replace_focused_text(&mut harness, "1.4");
    press(harness.visual, "enter");
    wait_for_page_writes(&writes, 1);
    wait_for_page_idle(&mut harness, &root);
    assert_eq!(writes.lock().expect("page writes")[0].line_height, 1.4);

    // An invalid scrollback draft also keeps the accepted font draft and
    // focus on the field the user must correct.
    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(&mut harness, "scrollback");
    press(harness.visual, "enter");
    replace_focused_text(&mut harness, "20000001");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(
            page.font_spacing_values(cx),
            ("1.4".to_string(), "3".to_string())
        );
        assert_eq!(page.scrollback_value(cx), "20000001");
        assert!(page_scrollback_focused(&root, window, cx));
        assert_eq!(config.read(cx).config().scrollback_lines, 4321);
    });
    assert_eq!(writes.lock().expect("page writes").len(), 1);

    replace_focused_text(&mut harness, "4400");
    press(harness.visual, "enter");
    wait_for_page_writes(&writes, 2);
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|_, cx| {
        assert_eq!(harness.config.read(cx).scrollback_lines(), 4400);
        assert_eq!(harness.config.read(cx).config().terminal_line_height, 1.4);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_failed_page_save_keeps_every_draft_and_retry_succeeds(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let failures = Arc::new(AtomicUsize::new(1));
    let recorded = Arc::clone(&attempts);
    let remaining = Arc::clone(&failures);
    let writer: TerminalPageWriter = Arc::new(move |settings| {
        recorded
            .lock()
            .expect("page attempts")
            .push(settings.clone());
        if remaining.fetch_sub(1, Ordering::SeqCst) > 0 {
            return Err("the config disk is full".to_string());
        }
        Ok(settings)
    });
    let mut harness = harness_with_page_writer(cx, AppConfig::default(), writer);
    let root = harness.root.clone();
    let config = harness.config.clone();
    let saved_family = harness
        .visual
        .update(|_, cx| config.read(cx).config().font_family.clone());
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2000.0)));
    open_font_draft(&mut harness, &root);

    press(harness.visual, "enter");
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.page_error(), Some("the config disk is full"));
        assert_eq!(
            page.font_error(cx).as_deref(),
            Some("the config disk is full")
        );
        assert_eq!(page.font_values(cx).0, "Menlo, monospace");
        assert_eq!(
            page.font_spacing_values(cx),
            ("1.5".to_string(), "3".to_string())
        );
        assert_eq!(page.scrollback_value(cx), "4321");
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
        assert_eq!(config.read(cx).config().font_family, saved_family);
        assert_eq!(config.read(cx).config().terminal_line_height, 1.0);
    });
    assert_eq!(attempts.lock().expect("page attempts").len(), 1);

    press(harness.visual, "enter");
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.page_saved());
        assert!(page.page_error().is_none());
        assert!(page.font_error(cx).is_none());
        assert_eq!(config.read(cx).scrollback_lines(), 4321);
        assert_eq!(config.read(cx).config().font_family, "Menlo, monospace");
        assert_eq!(config.read(cx).config().font_size, 20);
        assert_eq!(config.read(cx).config().terminal_line_height, 1.5);
        assert_eq!(config.read(cx).config().terminal_letter_spacing, 3);
    });
    assert_eq!(attempts.lock().expect("page attempts").len(), 2);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn search_focuses_spacing_and_scrollback_after_the_panel_scrolled(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(800.0), gpui_kit::px(636.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    // Scrolling the normal viewport down must not change which field a
    // search result opens.
    let before = page_bounds(
        harness.visual,
        settings_page::SETTINGS_SCROLLBACK_SECTION_ID,
    )
    .origin
    .y
    .as_f32();
    let panel = page_bounds(harness.visual, settings_page::SETTINGS_PANEL_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        window.dispatch_event(
            ScrollEvent {
                position: panel.center(),
                delta: ScrollDelta::Pixels(gpui_kit::point(
                    gpui_kit::px(0.0),
                    gpui_kit::px(-900.0),
                )),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    });
    let after = page_bounds(
        harness.visual,
        settings_page::SETTINGS_SCROLLBACK_SECTION_ID,
    )
    .origin
    .y
    .as_f32();
    assert!(
        after < before - 100.0,
        "the panel should scroll before the search result opens"
    );

    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(&mut harness, "line height");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        assert!(
            page_font_line_height_focused(&root, window, cx),
            "Line height should focus the line-height field"
        );
        assert!(!page_font_family_focused(&root, window, cx));
        assert_eq!(
            root.read(cx)
                .ui
                .settings_page
                .read(cx)
                .font_spacing_values(cx),
            ("1".to_string(), "0".to_string())
        );
    });

    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(&mut harness, "letter spacing");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        assert!(
            page_font_letter_spacing_focused(&root, window, cx),
            "Letter spacing should focus the letter-spacing field"
        );
        assert!(!page_font_line_height_focused(&root, window, cx));
    });

    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(&mut harness, "scrollback");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        assert!(page_scrollback_focused(&root, window, cx));
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}
