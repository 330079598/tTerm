use super::*;

#[gpui_kit::test]
fn titlebar_click_opens_one_route_and_back_restores_the_exact_pane(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first_model = Arc::clone(&harness.first_model);
    let second_model = Arc::clone(&harness.second_model);
    send_marker(harness.visual, &first_model, "beforefirst");
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| workspace.focus_pane(2, window, cx));
    });
    send_marker(harness.visual, &second_model, "beforesecond");
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.focus_pane(1, window, cx);
            assert_panes(workspace, &[1, 2], 1, window, cx);
        });
    });
    let before = settled_outputs(&[first_model.as_ref(), second_model.as_ref()]);
    assert!(before[0].contains("beforefirst"));
    assert!(before[1].contains("beforesecond"));

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
        assert!(workspace.factory.sessions.tab("gpui-1").is_some());
        assert!(Rc::ptr_eq(&first, &tab_binding(workspace, 1)));
        assert!(Rc::ptr_eq(&second, &tab_binding(workspace, 2)));
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 1);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.35, 0.65]));
        assert_eq!(output(&first_model), before[0]);
        assert_eq!(output(&second_model), before[1]);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        let focus = pane_focus(workspace, 1, cx);
        assert!(focus.is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 1);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.35, 0.65]));
        assert_eq!(output(&first_model), before[0]);
        assert_eq!(output(&second_model), before[1]);
    });

    send_marker(harness.visual, &first_model, "backtopane");
    assert!(!output(&second_model).contains("backtopane"));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn keymap_opens_the_singleton_route_and_escape_returns_input(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let second_model = Arc::clone(&harness.second_model);
    let first_model = Arc::clone(&harness.first_model);
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.focus_pane(2, window, cx);
            assert_panes(workspace, &[1, 2], 2, window, cx);
        });
    });

    press(harness.visual, &format!("{}-,", modifier()));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 2, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 2);
    });
    assert_eq!(settings_page_count(harness.visual), 1);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(!settings_page_visible(harness.visual));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 2, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 2);
    });

    press(harness.visual, &format!("{}-,", modifier()));
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 2, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 2);
    });

    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 2, window, cx);
        assert!(pane_focus(workspace, 2, cx).is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 2);
    });

    send_marker(harness.visual, &second_model, "keymapreturn");
    assert!(!output(&first_model).contains("keymapreturn"));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn remapped_settings_key_replaces_the_default_chord(cx: &mut TestAppContext) {
    let mut config = AppConfig::default();
    config
        .keymap
        .bindings
        .insert("settings.open".into(), Some(vec!["mod+alt+p".into()]));
    let harness = harness(cx, config);
    let root = harness.root.clone();
    let first_model = Arc::clone(&harness.first_model);
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);

    press(harness.visual, &format!("{}-,", modifier()));
    assert!(!settings_page_visible(harness.visual));
    harness
        .visual
        .update(|_, cx| assert!(!root.read(cx).settings_is_open()));

    let remapped = format!("{}-alt-p", modifier());
    press(harness.visual, &remapped);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });
    assert_eq!(settings_page_count(harness.visual), 1);

    press(harness.visual, &format!("{}-,", modifier()));
    harness
        .visual
        .update(|_, cx| assert!(root.read(cx).settings_is_open()));

    press(harness.visual, &remapped);
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    press(harness.visual, &remapped);
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });
    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    send_marker(harness.visual, &first_model, "remapreturn");
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn settings_input_does_not_leak_and_background_output_keeps_running(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first_model = Arc::clone(&harness.first_model);
    let second_model = Arc::clone(&harness.second_model);
    send_marker(harness.visual, &first_model, "beforeinput");
    let before = settled_outputs(&[first_model.as_ref()])[0].clone();
    let background = Rc::clone(&harness.second);
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert!(!pane_focus(workspace, 1, cx).is_focused(window));
        assert!(!pane_focus(workspace, 2, cx).is_focused(window));
    });

    harness.visual.simulate_input("leakprobe");
    harness.visual.update(|window, cx| window.render_frame(cx));
    std::thread::sleep(Duration::from_millis(250));
    assert!(!output(&first_model).contains("leakprobe"));
    assert!(!output(&second_model).contains("leakprobe"));
    harness.visual.update(|_, cx| {
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).search_query(),
            "leakprobe"
        );
    });

    let probe = if cfg!(windows) {
        "echo bgprobe\r\n"
    } else {
        "echo bgprobe\n"
    };
    background
        .session()
        .expect("the background session")
        .write_input(probe.as_bytes())
        .expect("the background PTY accepts input");
    wait_until("the hidden background session to print bgprobe", || {
        output(&second_model).contains("bgprobe")
    });
    assert!(!output(&first_model).contains("bgprobe"));
    harness
        .visual
        .update(|_, cx| assert!(root.read(cx).settings_is_open()));

    press(harness.visual, "escape");
    assert!(settings_page_visible(harness.visual));
    harness.visual.update(|_, cx| {
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
    });
    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
        assert_eq!(output(&first_model), before);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    send_marker(harness.visual, &first_model, "afterbackground");
    assert!(output(&first_model).contains("afterbackground"));
    assert!(!output(&second_model).contains("afterbackground"));
    harness.visual.update(|window, _| window.remove_window());
}
