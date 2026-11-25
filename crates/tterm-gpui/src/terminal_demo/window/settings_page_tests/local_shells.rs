use super::*;

#[gpui_kit::test]
fn local_shells_add_and_edit_persist_with_explicit_save(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let config = harness.config.clone();
    let shell_writes = Arc::clone(&harness.local_shell_writes);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, SETTINGS_TAB_LOCAL_SHELLS_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.active_tab(), SettingsTab::LocalShells);
        assert!(page.local_shells_editor(cx).is_none());
    });
    assert!(
        harness
            .visual
            .debug_bounds(LOCAL_SHELLS_SECTION_ID)
            .is_some()
    );

    click(harness.visual, LOCAL_SHELLS_ADD_ID);
    replace_focused_text(&mut harness, "/bin/nu");
    click_id(harness.visual, NAME_INPUT_ID);
    replace_focused_text(&mut harness, "my nu");
    click_id(harness.visual, ARGS_INPUT_ID);
    replace_focused_text(&mut harness, "--login");
    click_id(harness.visual, LOCAL_SHELLS_SAVE_ID);
    wait_for_local_shells(&mut harness, 1);

    let added_id = harness.visual.update(|_, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.local_shells_editor(cx).is_none());
        let shells = &config.read(cx).config().local_shells;
        assert_eq!(shells.len(), 1);
        assert_eq!(shells[0].path, "/bin/nu");
        assert_eq!(shells[0].name, "my nu");
        assert_eq!(shells[0].args, "--login");
        shells[0].id.clone()
    });
    assert_eq!(shell_writes.lock().expect("local shell writes").len(), 1);

    click_id(harness.visual, &format!("{EDIT_PREFIX}0"));
    harness.visual.update(|_, cx| {
        assert_eq!(
            root.read(cx)
                .ui
                .settings_page
                .read(cx)
                .local_shells_editor(cx),
            Some((
                "/bin/nu".to_string(),
                "my nu".to_string(),
                "--login".to_string()
            ))
        );
    });
    click_id(harness.visual, NAME_INPUT_ID);
    replace_focused_text(&mut harness, "renamed nu");
    click_id(harness.visual, LOCAL_SHELLS_SAVE_ID);
    wait_for_local_shell_writes(&mut harness, 2);
    harness.visual.update(|window, cx| window.render_frame(cx));

    harness.visual.update(|_, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.local_shells_editor(cx).is_none());
        let shells = &config.read(cx).config().local_shells;
        assert_eq!(shells.len(), 1, "editing must not add a second shell");
        assert_eq!(shells[0].id, added_id);
        assert_eq!(shells[0].name, "renamed nu");
    });
    assert_eq!(shell_writes.lock().expect("local shell writes").len(), 2);

    click(harness.visual, SETTINGS_TAB_TERMINAL_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SCROLLBACK_INPUT_ID)
            .is_some()
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn local_shells_lists_configured_shells_and_edits_them(cx: &mut TestAppContext) {
    let existing = LocalShell {
        id: "existing-nu".to_string(),
        name: "my nu".to_string(),
        path: "/bin/nu".to_string(),
        args: "--login".to_string(),
    };
    let harness = harness(
        cx,
        AppConfig {
            local_shells: vec![existing.clone()],
            ..AppConfig::default()
        },
    );
    let root = harness.root.clone();

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, SETTINGS_TAB_LOCAL_SHELLS_ID);
    click_id(harness.visual, &format!("{EDIT_PREFIX}0"));

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(
            page.local_shells_editor(cx),
            Some((
                existing.path.clone(),
                existing.name.clone(),
                existing.args.clone()
            ))
        );
        assert!(page.local_shells_error(cx).is_none());
    });
    click_id(harness.visual, crate::local_shells_setting::CANCEL_ID);
    harness.visual.update(|_, cx| {
        assert!(
            root.read(cx)
                .ui
                .settings_page
                .read(cx)
                .local_shells_editor(cx)
                .is_none()
        );
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn local_shells_save_failure_keeps_the_draft_and_retry_succeeds(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&attempts);
    let failures = Arc::new(std::sync::atomic::AtomicUsize::new(1));
    let remaining = Arc::clone(&failures);
    let local_writer: LocalShellsWriter = Arc::new(move |saved| {
        recorded
            .lock()
            .expect("local shell attempts")
            .push(saved.clone());
        if remaining.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) > 0 {
            return Err("the config disk is full".to_string());
        }
        Ok(SavedLocalShells {
            local_shells: vec![saved],
            default_shell: DefaultShellSettings::default(),
        })
    });
    let local_writes = Arc::new(Mutex::new(Vec::new()));
    let mut harness = harness_with_writers(
        cx,
        AppConfig::default(),
        Arc::new(|_| Ok(())),
        Arc::new(Mutex::new(Vec::new())),
        local_writer,
        local_writes,
        None,
    );
    let root = harness.root.clone();
    let config = harness.config.clone();

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, SETTINGS_TAB_LOCAL_SHELLS_ID);
    click(harness.visual, LOCAL_SHELLS_ADD_ID);
    replace_focused_text(&mut harness, "/bin/nu");
    click_id(harness.visual, LOCAL_SHELLS_SAVE_ID);
    wait_for_local_shells_error(&mut harness);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(
            page.local_shells_editor(cx),
            Some(("/bin/nu".to_string(), String::new(), String::new()))
        );
        assert_eq!(
            page.local_shells_error(cx).as_deref(),
            Some("the config disk is full")
        );
        assert!(!page.local_shells_saving(cx));
        assert!(config.read(cx).config().local_shells.is_empty());
    });

    click_id(harness.visual, LOCAL_SHELLS_SAVE_ID);
    wait_for_local_shells(&mut harness, 1);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.local_shells_editor(cx).is_none());
        assert!(page.local_shells_error(cx).is_none());
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).config().local_shells[0].path, "/bin/nu");
    });
    assert_eq!(attempts.lock().expect("local shell attempts").len(), 2);
    assert!(
        harness.visual.debug_bounds(LOCAL_SHELLS_ERROR_ID).is_none(),
        "a successful retry should clear the error"
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn local_shells_page_is_searchable_and_scrollback_stays_reachable(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    type_search(&mut harness, "local shells");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(
            page.result_titles(cx)
                .iter()
                .any(|title| title == "Local shells")
        );
        window.render_frame(cx);
    });
    click(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_RESULT_0_ID,
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.active_tab(), SettingsTab::LocalShells);
        assert_eq!(page.search_query(), "");
    });
    assert!(
        harness
            .visual
            .debug_bounds(LOCAL_SHELLS_SECTION_ID)
            .is_some()
    );

    click(harness.visual, SETTINGS_TAB_TERMINAL_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).active_tab(),
            SettingsTab::Terminal
        );
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SCROLLBACK_INPUT_ID)
            .is_some()
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}
