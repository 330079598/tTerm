use super::*;

#[gpui_kit::test]
fn search_finds_the_scrollback_setting_and_opens_it(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(settings_page_visible(harness.visual));

    type_search(&mut harness, "scrollback");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_query(), "scrollback");
        assert_eq!(page.search_result_count(cx), 1);
        assert_eq!(page.result_titles(cx), vec!["Scrollback Lines".to_string()]);
        window.render_frame(cx);
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULT_0_ID)
            .is_some()
    );
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_EMPTY_ID)
            .is_none()
    );

    press(harness.visual, "down");
    press(harness.visual, "up");
    harness
        .visual
        .update(|_, cx| assert_eq!(root.read(cx).ui.settings_page.read(cx).active_index(), 0));

    press(harness.visual, "enter");
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULTS_ID)
            .is_none()
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_value(cx), "");
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULT_0_ID)
            .is_none()
    );

    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn search_reports_no_match_and_clears_before_closing(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    type_search(&mut harness, "zzz");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_result_count(cx), 0);
        assert_eq!(
            page.empty_message(cx),
            "No settings match \u{201c}zzz\u{201d}"
        );
        window.render_frame(cx);
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_EMPTY_ID)
            .is_some()
    );
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULT_0_ID)
            .is_none()
    );

    click(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_CLEAR_ID,
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_EMPTY_ID)
            .is_none()
    );
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_PANEL_ID)
            .is_some()
    );

    type_search(&mut harness, "scrollback");
    press(harness.visual, "escape");
    assert!(settings_page_visible(harness.visual));
    harness
        .visual
        .update(|_, cx| assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), ""));
    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn search_accepts_chinese_and_mouse_selection(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|_, cx| {
        let locale = root.read(cx).ui.locale.clone();
        locale.update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
    });

    type_search(&mut harness, "回滚");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_query(), "回滚");
        assert_eq!(page.result_titles(cx), vec!["回滚行数".to_string()]);
        window.render_frame(cx);
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULT_0_ID)
            .is_some()
    );

    click(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_RESULT_0_ID,
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
        assert!(root.read(cx).settings_is_open());
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_PANEL_ID)
            .is_some()
    );

    harness.visual.update(|_, cx| {
        let locale = root.read(cx).ui.locale.clone();
        locale.update(cx, |locale, cx| locale.set_language(Language::En, cx));
    });
    click(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_FIELD_ID,
    );
    type_search(&mut harness, "scrollback");
    click(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_RESULT_0_ID,
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_PANEL_ID)
            .is_some()
    );

    click(
        harness.visual,
        super::settings_page::SETTINGS_OPEN_BUTTON_ID,
    );
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn arrow_keys_move_between_the_page_and_its_setting(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    type_search(&mut harness, "in");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(
            page.search_result_count(cx) >= 2,
            "the arrow test needs at least two results"
        );
        assert_eq!(page.active_index(), 0);
        window.render_frame(cx);
    });

    press(harness.visual, "down");
    harness
        .visual
        .update(|_, cx| assert_eq!(root.read(cx).ui.settings_page.read(cx).active_index(), 1));
    press(harness.visual, "up");
    harness
        .visual
        .update(|_, cx| assert_eq!(root.read(cx).ui.settings_page.read(cx).active_index(), 0));
    press(harness.visual, "down");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
        assert!(root.read(cx).settings_is_open());
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULT_0_ID)
            .is_none()
    );

    harness.visual.update(|window, _| window.remove_window());
}
