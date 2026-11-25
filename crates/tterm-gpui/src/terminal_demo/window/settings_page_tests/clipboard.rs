//! Route tests for the Terminal behavior switches.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::page_save::{echo_page_writer, wait_for_page_idle, wait_for_page_writes};
use super::*;
use crate::clipboard_setting::{
    CONFIRM_MULTILINE_PASTE_SWITCH_ID, COPY_ON_SELECT_SWITCH_ID, ClipboardDraft,
    MAC_OPTION_IS_META_ROW_ID, MAC_OPTION_IS_META_SWITCH_ID, OSC52_CLIPBOARD_WRITE_SWITCH_ID,
    RIGHT_CLICK_PASTE_SWITCH_ID, SECTION_ID,
};

/// A configuration whose five behavior preferences are all off, so a test
/// can show each switch changing exactly one draft.
fn clipboard_config() -> AppConfig {
    AppConfig {
        mac_option_is_meta: false,
        confirm_multiline_paste: false,
        copy_on_select: false,
        right_click_paste: false,
        osc52_clipboard_write: false,
        ..AppConfig::default()
    }
}

fn clipboard_draft() -> ClipboardDraft {
    ClipboardDraft::from(&clipboard_config())
}

fn page_clipboard_values(root: &Entity<Prototype>, cx: &App) -> ClipboardDraft {
    root.read(cx).ui.settings_page.read(cx).clipboard_values(cx)
}

fn clipboard_values(harness: &mut Harness<'_>, root: &Entity<Prototype>) -> ClipboardDraft {
    harness
        .visual
        .update(|_, cx| page_clipboard_values(root, cx))
}

fn page_clipboard_error(root: &Entity<Prototype>, cx: &App) -> Option<String> {
    root.read(cx).ui.settings_page.read(cx).clipboard_error(cx)
}

fn space(visual: &mut VisualTestContext) {
    let keystroke = gpui_kit::Keystroke::parse("space").unwrap();
    visual.simulate_event(gpui_kit::KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.simulate_event(gpui_kit::KeyUpEvent { keystroke });
    visual.update(|window, cx| window.render_frame(cx));
}

fn section_in_panel(visual: &mut VisualTestContext) -> bool {
    let panel = page_bounds(visual, settings_page::SETTINGS_PANEL_ID);
    let section = page_bounds(visual, SECTION_ID);
    section.origin.y >= panel.origin.y && section.origin.y < panel.origin.y + panel.size.height
}

fn page_section_is_visible(harness: &mut Harness<'_>) -> bool {
    section_in_panel(harness.visual)
}

fn search_titles(harness: &mut Harness<'_>, root: &Entity<Prototype>, query: &str) -> Vec<String> {
    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(harness, query);
    harness
        .visual
        .update(|_, cx| root.read(cx).ui.settings_page.read(cx).result_titles(cx))
}

#[gpui_kit::test]
fn switches_draft_until_the_page_save_and_reload_on_reopen(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut harness = harness_with_page_writer(
        cx,
        clipboard_config(),
        echo_page_writer(Arc::clone(&writes)),
    );
    let root = harness.root.clone();
    let config = harness.config.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    assert_eq!(clipboard_values(&mut harness, &root), clipboard_draft());

    click_id(harness.visual, CONFIRM_MULTILINE_PASTE_SWITCH_ID);
    assert_eq!(
        clipboard_values(&mut harness, &root),
        ClipboardDraft {
            confirm_multiline_paste: true,
            ..clipboard_draft()
        }
    );

    // The switch takes focus from the click, and Space must toggle the same
    // draft without writing the configuration.
    space(harness.visual);
    assert_eq!(clipboard_values(&mut harness, &root), clipboard_draft());
    space(harness.visual);
    assert_eq!(
        clipboard_values(&mut harness, &root),
        ClipboardDraft {
            confirm_multiline_paste: true,
            ..clipboard_draft()
        }
    );

    click_id(harness.visual, COPY_ON_SELECT_SWITCH_ID);
    click_id(harness.visual, RIGHT_CLICK_PASTE_SWITCH_ID);
    click_id(harness.visual, OSC52_CLIPBOARD_WRITE_SWITCH_ID);
    let expected_draft = ClipboardDraft {
        confirm_multiline_paste: true,
        copy_on_select: true,
        right_click_paste: true,
        osc52_clipboard_write: true,
        ..clipboard_draft()
    };
    assert_eq!(clipboard_values(&mut harness, &root), expected_draft);
    assert!(writes.lock().expect("page writes").is_empty());
    harness.visual.update(|_, cx| {
        let config = config.read(cx).config();
        assert!(!config.confirm_multiline_paste);
        assert!(!config.copy_on_select);
        assert!(!config.right_click_paste);
        assert!(!config.osc52_clipboard_write);
    });

    click(harness.visual, super::settings_page::SETTINGS_SAVE_ID);
    wait_for_page_writes(&writes, 1);
    wait_for_page_idle(&mut harness, &root);
    let expected = TerminalPageSettings {
        confirm_multiline_paste: true,
        copy_on_select: true,
        right_click_paste: true,
        osc52_clipboard_write: true,
        ..TerminalPageSettings::from(&clipboard_config())
    };
    assert_eq!(
        *writes.lock().expect("page writes"),
        vec![expected],
        "one Save writes the whole Terminal page"
    );
    harness.visual.update(|_, cx| {
        let config = config.read(cx).config();
        assert!(config.confirm_multiline_paste);
        assert!(config.copy_on_select);
        assert!(config.right_click_paste);
        assert!(config.osc52_clipboard_write);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|_, cx| {
        assert_eq!(page_clipboard_values(&root, cx), expected_draft);
        assert!(page_clipboard_error(&root, cx).is_none());
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn option_as_meta_is_visible_and_searchable_only_on_macos(cx: &mut TestAppContext) {
    let mut harness = harness(cx, clipboard_config());
    let root = harness.root.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    let titles = search_titles(&mut harness, &root, "option as meta");
    if cfg!(target_os = "macos") {
        assert_eq!(titles, vec!["Use Option as Meta key".to_string()]);
        press(harness.visual, "enter");
        assert!(section_in_panel(harness.visual));
    } else {
        assert!(titles.is_empty());
    }

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn search_finds_each_switch_and_scrolls_its_section_into_view(cx: &mut TestAppContext) {
    let mut harness = harness(cx, clipboard_config());
    let root = harness.root.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(800.0), gpui_kit::px(636.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    let mut cases = vec![
        (
            "confirm multiline paste",
            "Confirm multiline paste",
            CONFIRM_MULTILINE_PASTE_SWITCH_ID,
        ),
        ("copy on select", "Copy on select", COPY_ON_SELECT_SWITCH_ID),
        (
            "right-click to paste",
            "Right-click to paste",
            RIGHT_CLICK_PASTE_SWITCH_ID,
        ),
        (
            "let programs copy",
            "Let programs copy to the clipboard",
            OSC52_CLIPBOARD_WRITE_SWITCH_ID,
        ),
    ];
    if cfg!(target_os = "macos") {
        cases.insert(
            0,
            (
                "option as meta",
                "Use Option as Meta key",
                MAC_OPTION_IS_META_SWITCH_ID,
            ),
        );
    }

    for (query, title, switch_id) in cases {
        click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
        type_search(&mut harness, query);
        harness.visual.update(|_, cx| {
            assert_eq!(
                root.read(cx).ui.settings_page.read(cx).result_titles(cx),
                vec![title.to_string()]
            );
        });
        press(harness.visual, "enter");
        harness.visual.update(|_, cx| {
            assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
        });
        assert!(
            page_section_is_visible(&mut harness),
            "{title} should scroll its switch section into the panel"
        );
        let panel = page_bounds(harness.visual, settings_page::SETTINGS_PANEL_ID);
        let control = page_bounds(harness.visual, switch_id);
        assert!(
            control.origin.y >= panel.origin.y
                && control.origin.y + control.size.height <= panel.origin.y + panel.size.height,
            "{title} should be inside the settings viewport"
        );
    }

    harness.visual.update(|_, cx| {
        let locale = root.read(cx).ui.locale.clone();
        locale.update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
    });
    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(&mut harness, "多行粘贴");
    harness.visual.update(|_, cx| {
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).result_titles(cx),
            vec!["多行粘贴前确认".to_string()]
        );
    });
    press(harness.visual, "enter");
    assert!(page_section_is_visible(&mut harness));

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_hidden_mac_option_preference_survives_saving_other_switches(cx: &mut TestAppContext) {
    let mut config = clipboard_config();
    config.mac_option_is_meta = true;
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut harness = harness_with_page_writer(cx, config, echo_page_writer(Arc::clone(&writes)));
    let root = harness.root.clone();
    let config = harness.config.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    if !cfg!(target_os = "macos") {
        harness.visual.update(|window, cx| window.render_frame(cx));
        assert!(
            harness
                .visual
                .debug_bounds(MAC_OPTION_IS_META_ROW_ID)
                .is_none(),
            "the macOS-only row must not render elsewhere"
        );
        assert!(search_titles(&mut harness, &root, "option as meta").is_empty());
        press(harness.visual, "escape");
    }

    click_id(harness.visual, OSC52_CLIPBOARD_WRITE_SWITCH_ID);
    click(harness.visual, super::settings_page::SETTINGS_SAVE_ID);
    wait_for_page_writes(&writes, 1);
    wait_for_page_idle(&mut harness, &root);

    let saved = writes.lock().expect("page writes")[0].clone();
    assert!(
        saved.mac_option_is_meta,
        "the hidden preference must travel through the page save"
    );
    assert!(saved.osc52_clipboard_write);
    harness.visual.update(|_, cx| {
        assert!(config.read(cx).config().mac_option_is_meta);
        assert!(config.read(cx).config().osc52_clipboard_write);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_failed_save_keeps_the_clipboard_draft_and_retry_writes_it(cx: &mut TestAppContext) {
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
    let mut harness = harness_with_page_writer(cx, clipboard_config(), writer);
    let root = harness.root.clone();
    let config = harness.config.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click_id(harness.visual, CONFIRM_MULTILINE_PASTE_SWITCH_ID);
    click_id(harness.visual, COPY_ON_SELECT_SWITCH_ID);
    click_id(harness.visual, RIGHT_CLICK_PASTE_SWITCH_ID);
    click_id(harness.visual, OSC52_CLIPBOARD_WRITE_SWITCH_ID);
    let expected_draft = ClipboardDraft {
        confirm_multiline_paste: true,
        copy_on_select: true,
        right_click_paste: true,
        osc52_clipboard_write: true,
        ..clipboard_draft()
    };

    click(harness.visual, super::settings_page::SETTINGS_SAVE_ID);
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.page_error(), Some("the config disk is full"));
        assert_eq!(
            page_clipboard_error(&root, cx).as_deref(),
            Some("the config disk is full")
        );
        assert_eq!(page_clipboard_values(&root, cx), expected_draft);
        let config = config.read(cx).config();
        assert!(!config.confirm_multiline_paste);
        assert!(!config.copy_on_select);
        assert!(!config.right_click_paste);
        assert!(!config.osc52_clipboard_write);
    });
    assert_eq!(attempts.lock().expect("page attempts").len(), 1);

    click(harness.visual, super::settings_page::SETTINGS_SAVE_ID);
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.page_saved());
        assert!(page.page_error().is_none());
        assert!(page_clipboard_error(&root, cx).is_none());
        assert_eq!(page_clipboard_values(&root, cx), expected_draft);
        let config = config.read(cx).config();
        assert!(config.confirm_multiline_paste);
        assert!(config.copy_on_select);
        assert!(config.right_click_paste);
        assert!(config.osc52_clipboard_write);
    });
    assert_eq!(attempts.lock().expect("page attempts").len(), 2);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|_, cx| {
        assert_eq!(page_clipboard_values(&root, cx), expected_draft);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}
