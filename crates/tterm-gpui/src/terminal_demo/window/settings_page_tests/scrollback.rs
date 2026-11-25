use super::*;

#[gpui_kit::test]
fn edited_scrollback_survives_reopening_and_stays_applied(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first_model = Arc::clone(&harness.first_model);
    let writes = Arc::clone(&harness.writes);
    let config = harness.config.clone();
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness
        .visual
        .update(|_, cx| assert!(root.read(cx).settings_is_open()));
    open_scrollback_from_search(&mut harness, &root);

    // A value outside the field's range stays a draft: pressing Enter must
    // not write it or shrink the live buffers.
    replace_focused_text(&mut harness, "20000001");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).scrollback_value(cx),
            "20000001"
        );
        assert!(page_scrollback_focused(&root, window, cx));
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SCROLLBACK_ERROR_ID)
            .is_some()
    );
    assert!(writes.lock().expect("scrollback writes").is_empty());

    replace_focused_text(&mut harness, "400");
    press(harness.visual, "enter");
    wait_for_save(&mut harness, 400);
    harness.visual.update(|_, cx| {
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).scrollback_lines(), 400);
    });
    assert_eq!(
        first_model.scrollback(),
        resolve_scrollback_lines(Some(400.0)) as usize
    );

    click(
        harness.visual,
        super::settings_page::SETTINGS_SCROLLBACK_INPUT_ID,
    );
    replace_focused_text(&mut harness, "800");
    press(harness.visual, "enter");
    wait_for_save(&mut harness, 800);
    harness
        .visual
        .update(|_, cx| assert_eq!(config.read(cx).scrollback_lines(), 800));
    assert_eq!(
        first_model.scrollback(),
        resolve_scrollback_lines(Some(800.0)) as usize
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    press(harness.visual, &format!("{}-,", modifier()));
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).scrollback_value(cx),
            "800"
        );
    });
    open_scrollback_from_search(&mut harness, &root);
    replace_focused_text(&mut harness, "1200");
    press(harness.visual, "enter");
    wait_for_save(&mut harness, 1200);
    harness.visual.update(|_, cx| {
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).scrollback_lines(), 1200);
    });
    assert_eq!(
        first_model.scrollback(),
        resolve_scrollback_lines(Some(1200.0)) as usize
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    assert_eq!(
        *writes.lock().expect("scrollback writes"),
        vec![400, 800, 1200]
    );
    send_marker(harness.visual, &first_model, "aftersetting");
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn keyboard_and_mouse_selection_focus_the_editable_value(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let config = harness.config.clone();
    let writes = Arc::clone(&harness.writes);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    open_scrollback_from_search(&mut harness, &root);
    replace_focused_text(&mut harness, "4321");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_value(cx), "");
        assert_eq!(page.scrollback_value(cx), "4321");
        assert!(page_scrollback_focused(&root, window, cx));
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
    });

    // The settings shortcut opens the route that is already showing. It must
    // keep the unsaved draft and its field instead of rebuilding the page.
    press(harness.visual, &format!("{}-,", modifier()));
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(page.scrollback_value(cx), "4321");
        assert_eq!(page.search_query(), "");
        assert!(page_scrollback_focused(&root, window, cx));
    });
    assert!(writes.lock().expect("scrollback writes").is_empty());

    press(harness.visual, "escape");
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
    });

    // The same result selected with the pointer must take the next typed
    // digits into the value, not into the search query.
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    type_search(&mut harness, "scrollback");
    click(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_RESULT_0_ID,
    );
    harness.visual.update(|window, cx| {
        assert!(page_scrollback_focused(&root, window, cx));
    });
    replace_focused_text(&mut harness, "8765");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_value(cx), "");
        assert_eq!(page.scrollback_value(cx), "8765");
        assert!(page_scrollback_focused(&root, window, cx));
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
    });
    assert!(writes.lock().expect("scrollback writes").is_empty());
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn tab_and_shift_tab_leave_the_value_for_the_page_focus_order(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    let page = harness
        .visual
        .update(|_, cx| root.read(cx).ui.settings_page.clone());
    harness.visual.update(|_, cx| {
        page.update(cx, |page, cx| page.set_font_system_fonts(Vec::new(), cx));
    });
    open_scrollback_from_search(&mut harness, &root);

    // The page order runs from search through the font fields to scrollback
    // and the single Save. Shift+Tab must leave the numeric value instead of
    // trapping focus, and the cycle must reach the search field again.
    press(harness.visual, "shift-tab");
    harness.visual.update(|window, cx| {
        assert!(
            !page_scrollback_focused(&root, window, cx),
            "Shift+Tab must leave the numeric field"
        );
    });
    let mut reached_search = false;
    for _ in 0..64 {
        if harness
            .visual
            .update(|window, cx| page_search_focused(&root, window, cx))
        {
            reached_search = true;
            break;
        }
        press(harness.visual, "shift-tab");
    }
    assert!(
        reached_search,
        "the page focus order should cycle back to the search field"
    );

    let mut reached_scrollback = false;
    for _ in 0..64 {
        press(harness.visual, "tab");
        if harness
            .visual
            .update(|window, cx| page_scrollback_focused(&root, window, cx))
        {
            reached_scrollback = true;
            break;
        }
    }
    assert!(
        reached_scrollback,
        "Tab should reach the scrollback field after the search field"
    );
    harness.visual.update(|_, cx| {
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).scrollback_value(cx),
            "10000"
        );
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn language_change_updates_placeholders_without_resetting_the_draft(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    let english_search = harness.visual.update(|_, cx| {
        root.read(cx)
            .ui
            .locale
            .read(cx)
            .text(keys::SETTINGS_SEARCH_PLACEHOLDER)
            .expect("the English search placeholder is embedded")
    });
    let english_scrollback = harness.visual.update(|_, cx| {
        root.read(cx)
            .ui
            .locale
            .read(cx)
            .text(keys::SCROLLBACK_SETTING_PLACEHOLDER)
            .expect("the English scrollback placeholder is embedded")
    });
    let english_font = harness.visual.update(|_, cx| {
        root.read(cx)
            .ui
            .locale
            .read(cx)
            .text(keys::FONT_SETTINGS_CUSTOM_FONT)
            .expect("the English font placeholder is embedded")
    });
    harness.visual.update(|_, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_placeholder(cx), english_search);
        assert_eq!(page.font_family_placeholder(cx), english_font);
    });

    open_scrollback_from_search(&mut harness, &root);
    replace_focused_text(&mut harness, "777");
    harness.visual.update(|_, cx| {
        assert_eq!(
            root.read(cx)
                .ui
                .settings_page
                .read(cx)
                .scrollback_placeholder(cx),
            english_scrollback
        );
    });

    let (chinese_search, chinese_scrollback, chinese_font) = harness.visual.update(|_, cx| {
        let locale = root.read(cx).ui.locale.clone();
        locale.update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
        (
            locale
                .read(cx)
                .text(keys::SETTINGS_SEARCH_PLACEHOLDER)
                .expect("the Chinese search placeholder is embedded"),
            locale
                .read(cx)
                .text(keys::SCROLLBACK_SETTING_PLACEHOLDER)
                .expect("the Chinese scrollback placeholder is embedded"),
            locale
                .read(cx)
                .text(keys::FONT_SETTINGS_CUSTOM_FONT)
                .expect("the Chinese font placeholder is embedded"),
        )
    });
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_placeholder(cx), chinese_search);
        assert_eq!(page.scrollback_placeholder(cx), chinese_scrollback);
        assert_eq!(page.font_family_placeholder(cx), chinese_font);
        assert!(!chinese_search.is_empty());
        assert_eq!(page.scrollback_value(cx), "777");
        assert_eq!(page.search_query(), "");
        assert!(page_scrollback_focused(&root, window, cx));
    });

    harness.visual.update(|_, cx| {
        let locale = root.read(cx).ui.locale.clone();
        locale.update(cx, |locale, cx| locale.set_language(Language::En, cx));
    });
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_placeholder(cx), english_search);
        assert_eq!(page.scrollback_placeholder(cx), english_scrollback);
        assert_eq!(page.font_family_placeholder(cx), english_font);
        assert_eq!(page.scrollback_value(cx), "777");
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn invalid_input_and_a_failed_save_recover_through_the_field(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let writer: ScrollbackWriter = Arc::new(move |lines| {
        let mut attempts = recorded.lock().expect("scrollback write attempts");
        attempts.push(lines);
        if attempts.len() == 1 {
            Err("the config disk is full".to_string())
        } else {
            Ok(())
        }
    });
    let mut harness = harness_with_writer(cx, AppConfig::default(), writer, Arc::clone(&writes));
    let root = harness.root.clone();
    let config = harness.config.clone();
    let first_model = Arc::clone(&harness.first_model);
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    open_scrollback_from_search(&mut harness, &root);

    replace_focused_text(&mut harness, "20000001");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).scrollback_value(cx),
            "20000001"
        );
        assert!(page_scrollback_focused(&root, window, cx));
    });
    wait_for_settings_error(harness.visual);
    assert!(writes.lock().expect("scrollback write attempts").is_empty());

    replace_focused_text(&mut harness, "500");
    press(harness.visual, "enter");
    wait_for_write_attempts(&writes, 1);
    wait_for_settings_error(harness.visual);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
        assert_eq!(
            first_model.scrollback(),
            resolve_scrollback_lines(Some(10_000.0)) as usize
        );
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).scrollback_value(cx),
            "500"
        );
        assert!(page_scrollback_focused(&root, window, cx));
        assert!(!config.read(cx).is_save_pending());
    });

    replace_focused_text(&mut harness, "400");
    press(harness.visual, "enter");
    wait_for_save(&mut harness, 400);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).scrollback_lines(), 400);
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SCROLLBACK_ERROR_ID)
            .is_none(),
        "a successful retry should clear the save error"
    );
    assert_eq!(
        first_model.scrollback(),
        resolve_scrollback_lines(Some(400.0)) as usize
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
    });
    assert_eq!(
        *writes.lock().expect("scrollback write attempts"),
        vec![500, 400]
    );
    harness.visual.update(|window, _| window.remove_window());
}
