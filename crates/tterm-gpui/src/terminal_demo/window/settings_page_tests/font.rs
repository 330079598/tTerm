use super::*;

#[gpui_kit::test]
fn font_draft_previews_before_the_single_page_save(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&attempts);
    let writer: TerminalFontWriter = Arc::new(move |family, size| {
        recorded
            .lock()
            .expect("font attempts")
            .push((family.clone(), size));
        Ok(tterm_core::config::terminal_font_file::SavedTerminalFont { family, size })
    });
    let mut harness = harness_with_font_writer(
        cx,
        AppConfig {
            font_family: "Menlo, monospace".to_string(),
            font_size: 14,
            ..AppConfig::default()
        },
        writer,
    );
    let root = harness.root.clone();
    let config = harness.config.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2000.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    type_search(&mut harness, "font family");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        assert!(
            page_font_family_focused(&root, window, cx),
            "the Font Family result should focus the family field"
        );
    });
    replace_focused_text(&mut harness, "\"Fira Code\", monospace");
    click_id(harness.visual, crate::font_setting::SIZE_INPUT_ID);
    replace_focused_text(&mut harness, "18");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(
            page.font_values(cx),
            ("\"Fira Code\", monospace".to_string(), "18".to_string())
        );
        assert_eq!(page.font_preview(cx), ("Fira Code".to_string(), 18));
        assert_eq!(config.read(cx).config().font_family, "Menlo, monospace");
        assert_eq!(config.read(cx).config().font_size, 14);
    });

    // The draft stays unpublished until the page is saved, and closing the
    // route without saving discards it rather than writing the focused field.
    press(harness.visual, "enter");
    wait_for_font_save(&mut harness, &root, "18");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.page_saved());
        assert_eq!(
            page.font_values(cx),
            ("\"Fira Code\", monospace".to_string(), "18".to_string())
        );
        assert_eq!(
            config.read(cx).config().font_family,
            "\"Fira Code\", monospace"
        );
        assert_eq!(config.read(cx).config().font_size, 18);
        assert!(page.font_error(cx).is_none());
        assert!(!page.page_saving());
    });
    assert_eq!(
        *attempts.lock().expect("font attempts"),
        vec![("\"Fira Code\", monospace".to_string(), 18)]
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn font_save_writes_and_reopening_reads_the_saved_values(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&attempts);
    let writer: TerminalFontWriter = Arc::new(move |family, size| {
        recorded
            .lock()
            .expect("font attempts")
            .push((family.clone(), size));
        Ok(tterm_core::config::terminal_font_file::SavedTerminalFont { family, size })
    });
    let mut harness = harness_with_font_writer(cx, AppConfig::default(), writer);
    let root = harness.root.clone();
    let config = harness.config.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2000.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    type_search(&mut harness, "font family");
    press(harness.visual, "enter");
    replace_focused_text(&mut harness, "\"Fira Code\", monospace");
    click_id(harness.visual, crate::font_setting::SIZE_INPUT_ID);
    replace_focused_text(&mut harness, "20");
    press(harness.visual, "enter");
    wait_for_font_save(&mut harness, &root, "20");

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.font_saved(cx));
        assert!(page.font_error(cx).is_none());
        assert_eq!(
            config.read(cx).config().font_family,
            "\"Fira Code\", monospace"
        );
        assert_eq!(config.read(cx).config().font_size, 20);
        for (_, tab) in root.read(cx).tabs.ordered() {
            let font = tab.view.read(cx).font();
            assert_eq!(font.families, ["Fira Code", "monospace"]);
            assert_eq!(font.size, 20.0);
        }
    });
    assert_eq!(
        *attempts.lock().expect("font attempts"),
        vec![("\"Fira Code\", monospace".to_string(), 20)]
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(!settings_page_visible(harness.visual));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(
            page.font_values(cx),
            ("\"Fira Code\", monospace".to_string(), "20".to_string())
        );
        assert_eq!(page.font_preview(cx), ("Fira Code".to_string(), 20));
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn font_save_failure_keeps_the_draft_and_retry_succeeds(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&attempts);
    let failures = Arc::new(std::sync::atomic::AtomicUsize::new(1));
    let remaining = Arc::clone(&failures);
    let writer: TerminalFontWriter = Arc::new(move |family, size| {
        recorded
            .lock()
            .expect("font attempts")
            .push((family.clone(), size));
        if remaining.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) > 0 {
            return Err("the config disk is full".to_string());
        }
        Ok(tterm_core::config::terminal_font_file::SavedTerminalFont { family, size })
    });
    let mut harness = harness_with_font_writer(cx, AppConfig::default(), writer);
    let root = harness.root.clone();
    let config = harness.config.clone();
    let saved_family = harness
        .visual
        .update(|_, cx| config.read(cx).config().font_family.clone());
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2000.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    type_search(&mut harness, "font size");
    press(harness.visual, "enter");
    replace_focused_text(&mut harness, "5");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.font_values(cx).1, "5");
        assert_eq!(
            page.font_error(cx).as_deref(),
            Some("Font size must be between 6 and 72.")
        );
        assert_eq!(config.read(cx).config().font_size, 14);
    });
    assert!(attempts.lock().expect("font attempts").is_empty());

    click_id(harness.visual, crate::font_setting::SIZE_INPUT_ID);
    replace_focused_text(&mut harness, "18");
    press(harness.visual, "enter");
    wait_for_font_page_error(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.font_values(cx).1, "18");
        assert_eq!(
            page.font_error(cx).as_deref(),
            Some("the config disk is full")
        );
        assert_eq!(config.read(cx).config().font_size, 14);
        for (_, tab) in root.read(cx).tabs.ordered() {
            assert_eq!(tab.view.read(cx).font().size, 14.0);
        }
    });

    press(harness.visual, "enter");
    wait_for_font_save(&mut harness, &root, "18");
    harness.visual.update(|_, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.font_error(cx).is_none());
        assert_eq!(page.font_values(cx).1, "18");
        assert_eq!(config.read(cx).config().font_size, 18);
        assert_eq!(config.read(cx).config().font_family, saved_family);
    });
    assert_eq!(attempts.lock().expect("font attempts").len(), 2);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn search_finds_the_font_controls_and_opens_their_fields(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    type_search(&mut harness, "font");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        let titles = page.result_titles(cx);
        assert!(titles.contains(&"Font Family".to_string()), "{titles:?}");
        assert!(titles.contains(&"Font Size".to_string()), "{titles:?}");
        window.render_frame(cx);
    });
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        assert!(
            page_font_family_focused(&root, window, cx),
            "the first font result should focus the family field"
        );
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
    });

    click_id(harness.visual, crate::font_setting::FAMILY_INPUT_ID);
    click(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_FIELD_ID,
    );
    type_search(&mut harness, "font size");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        assert!(
            page_font_size_focused(&root, window, cx),
            "the Font Size result should focus the custom size field"
        );
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn system_font_picker_filters_and_chooses_a_real_family(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let config = harness.config.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2000.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    let page = harness
        .visual
        .update(|_, cx| root.read(cx).ui.settings_page.clone());
    harness.visual.update(|_, cx| {
        page.update(cx, |page, cx| {
            page.set_font_system_fonts(
                vec![
                    "Menlo".to_string(),
                    "JetBrains Mono Nerd Font".to_string(),
                    "Fira Code Nerd Font".to_string(),
                ],
                cx,
            )
        });
    });
    harness.visual.update(|window, cx| window.render_frame(cx));
    assert!(
        harness
            .visual
            .debug_bounds(crate::font_setting::FONT_LIST_ID)
            .is_some()
    );

    click_id(harness.visual, crate::font_setting::SEARCH_INPUT_ID);
    harness.visual.simulate_input("fira");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(
            page.font_system_fonts(cx),
            vec![
                "Menlo".to_string(),
                "JetBrains Mono Nerd Font".to_string(),
                "Fira Code Nerd Font".to_string()
            ]
        );
    });
    click_id(
        harness.visual,
        &format!("{}0", crate::font_setting::FONT_ROW_PREFIX),
    );
    harness.visual.update(|_, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.font_values(cx).0, "\"Fira Code Nerd Font\", monospace");
        assert_eq!(
            config.read(cx).config().font_family,
            AppConfig::default().font_family
        );
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn chinese_search_finds_the_font_family_and_size(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|_, cx| {
        let locale = root.read(cx).ui.locale.clone();
        locale.update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
    });

    type_search(&mut harness, "字体");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        let titles = page.result_titles(cx);
        assert!(titles.contains(&"字体".to_string()), "{titles:?}");
        assert!(titles.contains(&"字体大小".to_string()), "{titles:?}");
        window.render_frame(cx);
    });
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        assert!(page_font_family_focused(&root, window, cx));
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}
