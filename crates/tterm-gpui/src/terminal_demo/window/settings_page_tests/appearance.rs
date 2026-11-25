//! Route tests for the cursor samples and the padding drafts.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::page_save::{echo_page_writer, wait_for_page_idle, wait_for_page_writes};
use super::*;
use crate::appearance_setting::{
    CURSOR_OPTION_PREFIX, PADDING_BOTTOM_INPUT_ID, PADDING_RIGHT_INPUT_ID, PADDING_SECTION_ID,
};

fn page_cursor_focused(root: &Entity<Prototype>, window: &Window, cx: &App) -> bool {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .appearance_cursor_focus(cx)
        .is_some_and(|focus| focus.is_focused(window))
}

fn page_padding_left_focused(root: &Entity<Prototype>, window: &Window, cx: &App) -> bool {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .appearance_padding_left_focus(cx)
        .is_some_and(|focus| focus.is_focused(window))
}

fn open_padding_from_search(harness: &mut Harness<'_>, root: &Entity<Prototype>) {
    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(harness, "terminal padding");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        assert!(
            page_padding_left_focused(root, window, cx),
            "the padding result should focus the left field"
        );
    });
}

#[gpui_kit::test]
fn cursor_and_padding_save_with_the_page_and_reload_on_reopen(cx: &mut TestAppContext) {
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
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    click_id(harness.visual, &format!("{CURSOR_OPTION_PREFIX}underline"));
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.appearance_cursor(cx), "underline");
        assert!(page_cursor_focused(&root, window, cx));
    });

    open_padding_from_search(&mut harness, &root);
    replace_focused_text(&mut harness, "12");
    click_id(harness.visual, PADDING_RIGHT_INPUT_ID);
    replace_focused_text(&mut harness, "14");
    click_id(harness.visual, PADDING_BOTTOM_INPUT_ID);
    replace_focused_text(&mut harness, "8");
    press(harness.visual, "enter");
    wait_for_page_writes(&writes, 1);
    wait_for_page_idle(&mut harness, &root);

    assert_eq!(
        *writes.lock().expect("page writes"),
        vec![TerminalPageSettings {
            cursor_style: "underline".to_string(),
            padding_left: 12,
            padding_right: 14,
            padding_bottom: 8,
            ..TerminalPageSettings::from(&AppConfig::default())
        }]
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.page_saved());
        assert!(page.page_error().is_none());
        assert!(page.appearance_error(cx).is_none());
        assert_eq!(
            page.appearance_padding(cx),
            ("12".to_string(), "14".to_string(), "8".to_string())
        );
        let config = config.read(cx).config();
        assert_eq!(config.cursor_style, "underline");
        assert_eq!(config.terminal_padding_left_px, 12);
        assert_eq!(config.terminal_padding_right_px, 14);
        assert_eq!(config.terminal_padding_bottom_px, 8);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.appearance_cursor(cx), "underline");
        assert_eq!(
            page.appearance_padding(cx),
            ("12".to_string(), "14".to_string(), "8".to_string())
        );
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_failed_appearance_save_keeps_the_draft_and_retry_succeeds(cx: &mut TestAppContext) {
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
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click_id(harness.visual, &format!("{CURSOR_OPTION_PREFIX}bar"));
    open_padding_from_search(&mut harness, &root);
    replace_focused_text(&mut harness, "20");
    click_id(harness.visual, PADDING_RIGHT_INPUT_ID);
    replace_focused_text(&mut harness, "0");
    click_id(harness.visual, PADDING_BOTTOM_INPUT_ID);
    replace_focused_text(&mut harness, "6");

    press(harness.visual, "enter");
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.page_error(), Some("the config disk is full"));
        assert_eq!(
            page.appearance_error(cx).as_deref(),
            Some("the config disk is full")
        );
        assert_eq!(page.appearance_cursor(cx), "bar");
        assert_eq!(
            page.appearance_padding(cx),
            ("20".to_string(), "0".to_string(), "6".to_string())
        );
        let config = config.read(cx).config();
        assert_eq!(config.cursor_style, "block");
        assert_eq!(config.terminal_padding_left_px, 6);
        assert_eq!(config.terminal_padding_right_px, 6);
        assert_eq!(config.terminal_padding_bottom_px, 4);
    });
    assert_eq!(attempts.lock().expect("page attempts").len(), 1);

    press(harness.visual, "enter");
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.page_saved());
        assert!(page.page_error().is_none());
        assert!(page.appearance_error(cx).is_none());
        let config = config.read(cx).config();
        assert_eq!(config.cursor_style, "bar");
        assert_eq!(config.terminal_padding_left_px, 20);
        assert_eq!(config.terminal_padding_right_px, 0);
        assert_eq!(config.terminal_padding_bottom_px, 6);
    });
    assert_eq!(attempts.lock().expect("page attempts").len(), 2);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn an_invalid_padding_keeps_every_draft_and_focuses_the_field(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut harness = harness_with_page_writer(
        cx,
        AppConfig::default(),
        echo_page_writer(Arc::clone(&writes)),
    );
    let root = harness.root.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click_id(harness.visual, &format!("{CURSOR_OPTION_PREFIX}bar"));
    open_padding_from_search(&mut harness, &root);
    replace_focused_text(&mut harness, "81");
    click_id(harness.visual, PADDING_RIGHT_INPUT_ID);
    replace_focused_text(&mut harness, "14");

    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(!page.page_saving());
        assert_eq!(
            page.appearance_error(cx).as_deref(),
            Some("Padding must be between 0 and 80 pixels.")
        );
        assert_eq!(page.appearance_cursor(cx), "bar");
        assert_eq!(
            page.appearance_padding(cx),
            ("81".to_string(), "14".to_string(), "4".to_string())
        );
        assert!(page_padding_left_focused(&root, window, cx));
    });
    assert!(writes.lock().expect("page writes").is_empty());

    replace_focused_text(&mut harness, "8");
    press(harness.visual, "enter");
    wait_for_page_writes(&writes, 1);
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|_, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.page_saved());
        assert!(page.appearance_error(cx).is_none());
        assert_eq!(
            page.appearance_padding(cx),
            ("8".to_string(), "14".to_string(), "4".to_string())
        );
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn search_scrolls_the_cursor_padding_and_preview_sections_into_view(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(800.0), gpui_kit::px(636.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(&mut harness, "cursor style");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        assert!(page_cursor_focused(&root, window, cx));
    });

    open_padding_from_search(&mut harness, &root);
    let panel = page_bounds(harness.visual, settings_page::SETTINGS_PANEL_ID);
    let padding = page_bounds(harness.visual, PADDING_SECTION_ID);
    assert!(
        padding.origin.y >= panel.origin.y && padding.origin.y < panel.origin.y + panel.size.height,
        "the padding section should be scrolled into the panel"
    );

    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(&mut harness, "preview");
    press(harness.visual, "enter");
    let preview = page_bounds(harness.visual, crate::font_setting::PREVIEW_ID);
    assert!(
        preview.origin.y >= panel.origin.y && preview.origin.y < panel.origin.y + panel.size.height,
        "the preview should be scrolled into the panel"
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}
