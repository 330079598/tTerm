//! Route tests for the command-mark and local shell-integration switches.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::page_save::{echo_page_writer, wait_for_page_idle, wait_for_page_writes};
use super::*;
use crate::command_marks_setting::{
    COMMAND_MARKS_ROW_ID, COMMAND_MARKS_SWITCH_ID, COPIED_ID, COPY_FISH_ID, COPY_POSIX_ID,
    CommandMarksDraft, LOCAL_INTEGRATION_ROW_ID, LOCAL_INTEGRATION_SWITCH_ID, SECTION_ID,
    local_integration_visible,
};
use tterm_core::remote_shell_integration::{FISH_SHELL_INTEGRATION, POSIX_SHELL_INTEGRATION};

/// A configuration whose marks are on and whose local integration is off, so
/// a test can show each switch changing exactly one draft.
fn marks_config() -> AppConfig {
    AppConfig {
        command_marks: true,
        local_unix_shell_integration: false,
        ..AppConfig::default()
    }
}

/// A configuration whose marks are off, so the local switch stays disabled
/// until the first switch turns them on.
fn disabled_marks_config() -> AppConfig {
    AppConfig {
        command_marks: false,
        local_unix_shell_integration: false,
        ..AppConfig::default()
    }
}

fn marks_draft(config: &AppConfig) -> CommandMarksDraft {
    CommandMarksDraft::from(config)
}

fn page_marks_values(root: &Entity<Prototype>, cx: &App) -> CommandMarksDraft {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .command_marks_values(cx)
}

fn marks_values(harness: &mut Harness<'_>, root: &Entity<Prototype>) -> CommandMarksDraft {
    harness.visual.update(|_, cx| page_marks_values(root, cx))
}

fn page_marks_error(root: &Entity<Prototype>, cx: &App) -> Option<String> {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .command_marks_error(cx)
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

fn search_titles(harness: &mut Harness<'_>, root: &Entity<Prototype>, query: &str) -> Vec<String> {
    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(harness, query);
    harness
        .visual
        .update(|_, cx| root.read(cx).ui.settings_page.read(cx).result_titles(cx))
}

/// Feeds one complete OSC 133 command to a model, independent of its PTY.
fn feed_command(model: &Terminal) {
    model.write(b"\x1b]133;A\x07$ cmd\r\n");
    model.write(b"\x1b]133;C\x07");
    model.write(b"out\r\n");
    model.write(b"\x1b]133;D;0\x07");
}

#[gpui_kit::test]
fn copy_actions_write_the_shared_scripts_and_leave_the_draft_alone(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut harness =
        harness_with_page_writer(cx, marks_config(), echo_page_writer(Arc::clone(&writes)));
    let root = harness.root.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    let before = marks_values(&mut harness, &root);
    harness.visual.update(|_, cx| {
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("sentinel".into()))
    });

    click(harness.visual, COPY_POSIX_ID);
    let copied = harness
        .visual
        .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(
        copied.as_deref(),
        Some(POSIX_SHELL_INTEGRATION),
        "the bash / zsh button copies the shared POSIX script"
    );
    assert_eq!(
        marks_values(&mut harness, &root),
        before,
        "copying does not change the draft"
    );
    assert!(writes.lock().expect("page writes").is_empty());
    harness.visual.update(|window, cx| window.render_frame(cx));
    assert!(
        harness.visual.debug_bounds(COPIED_ID).is_some(),
        "the row confirms that the script was copied"
    );

    click(harness.visual, COPY_FISH_ID);
    let copied = harness
        .visual
        .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(
        copied.as_deref(),
        Some(FISH_SHELL_INTEGRATION),
        "the fish button copies the shared fish script"
    );
    assert_eq!(marks_values(&mut harness, &root), before);
    assert!(writes.lock().expect("page writes").is_empty());

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn switches_draft_until_the_page_save_and_reload_on_reopen(cx: &mut TestAppContext) {
    let config = disabled_marks_config();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut harness =
        harness_with_page_writer(cx, config.clone(), echo_page_writer(Arc::clone(&writes)));
    let root = harness.root.clone();
    let state = harness.config.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    assert_eq!(marks_values(&mut harness, &root), marks_draft(&config));

    if local_integration_visible() {
        // The local switch is disabled while marks are off, so a click cannot
        // turn it on before the first switch does.
        click(harness.visual, LOCAL_INTEGRATION_SWITCH_ID);
        assert_eq!(marks_values(&mut harness, &root), marks_draft(&config));
    }

    click(harness.visual, COMMAND_MARKS_SWITCH_ID);
    assert_eq!(
        marks_values(&mut harness, &root),
        CommandMarksDraft {
            command_marks: true,
            ..marks_draft(&config)
        }
    );

    // The click focuses the switch, and Space toggles the same draft without
    // writing the configuration.
    space(harness.visual);
    assert_eq!(marks_values(&mut harness, &root), marks_draft(&config));
    space(harness.visual);
    let enabled = CommandMarksDraft {
        command_marks: true,
        ..marks_draft(&config)
    };
    assert_eq!(marks_values(&mut harness, &root), enabled);

    if local_integration_visible() {
        click(harness.visual, LOCAL_INTEGRATION_SWITCH_ID);
    }
    let expected_draft = CommandMarksDraft {
        command_marks: true,
        local_unix_shell_integration: local_integration_visible(),
    };
    assert_eq!(marks_values(&mut harness, &root), expected_draft);
    assert!(writes.lock().expect("page writes").is_empty());
    harness.visual.update(|_, cx| {
        let config = state.read(cx).config();
        assert!(!config.command_marks);
        assert!(!config.local_unix_shell_integration);
    });

    click(harness.visual, super::settings_page::SETTINGS_SAVE_ID);
    wait_for_page_writes(&writes, 1);
    wait_for_page_idle(&mut harness, &root);
    let mut expected = TerminalPageSettings::from(&config);
    expected.command_marks = true;
    expected.local_unix_shell_integration = local_integration_visible();
    assert_eq!(
        *writes.lock().expect("page writes"),
        vec![expected],
        "one Save writes the whole Terminal page"
    );
    harness.visual.update(|_, cx| {
        let config = state.read(cx).config();
        assert!(config.command_marks);
        assert_eq!(
            config.local_unix_shell_integration,
            local_integration_visible()
        );
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|_, cx| {
        assert_eq!(page_marks_values(&root, cx), expected_draft);
        assert!(page_marks_error(&root, cx).is_none());
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn search_finds_both_switches_and_scrolls_them_into_view(cx: &mut TestAppContext) {
    let mut harness = harness(cx, marks_config());
    let root = harness.root.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(800.0), gpui_kit::px(636.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    let titles = search_titles(&mut harness, &root, "mark commands");
    assert_eq!(
        titles,
        vec!["Mark commands (shell integration)".to_string()]
    );
    press(harness.visual, "enter");
    assert!(section_in_panel(harness.visual));
    let panel = page_bounds(harness.visual, settings_page::SETTINGS_PANEL_ID);
    let control = page_bounds(harness.visual, COMMAND_MARKS_SWITCH_ID);
    assert!(
        control.origin.y >= panel.origin.y
            && control.origin.y + control.size.height <= panel.origin.y + panel.size.height,
        "the command-mark switch should be inside the settings viewport"
    );
    // Opening the result focuses that switch, so Space edits its draft.
    space(harness.visual);
    assert_eq!(
        marks_values(&mut harness, &root),
        CommandMarksDraft {
            command_marks: false,
            ..marks_draft(&marks_config())
        }
    );
    space(harness.visual);
    assert_eq!(
        marks_values(&mut harness, &root),
        marks_draft(&marks_config())
    );

    if local_integration_visible() {
        let titles = search_titles(&mut harness, &root, "integration for local");
        assert_eq!(
            titles,
            vec!["Shell integration for local shells".to_string()]
        );
        press(harness.visual, "enter");
        assert!(section_in_panel(harness.visual));
        space(harness.visual);
        assert_eq!(
            marks_values(&mut harness, &root),
            CommandMarksDraft {
                command_marks: true,
                local_unix_shell_integration: true,
            }
        );
    } else {
        harness.visual.update(|window, cx| window.render_frame(cx));
        assert!(
            harness
                .visual
                .debug_bounds(LOCAL_INTEGRATION_ROW_ID)
                .is_none(),
            "the local integration row must not render on Windows"
        );
        assert!(
            search_titles(&mut harness, &root, "integration for local").is_empty(),
            "the local integration switch must not be searchable on Windows"
        );
        press(harness.visual, "escape");
    }
    assert!(
        harness.visual.debug_bounds(COMMAND_MARKS_ROW_ID).is_some(),
        "the command-mark row is present on every platform"
    );

    harness.visual.update(|_, cx| {
        let locale = root.read(cx).ui.locale.clone();
        locale.update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
    });
    let titles = search_titles(&mut harness, &root, "标记命令");
    assert!(
        titles.iter().any(|title| title == "标记命令（Shell 集成）"),
        "the Chinese label should be searchable: {titles:?}"
    );
    press(harness.visual, "enter");
    assert!(section_in_panel(harness.visual));

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn saving_marks_updates_open_panes_and_new_tabs(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut harness =
        harness_with_page_writer(cx, marks_config(), echo_page_writer(Arc::clone(&writes)));
    let root = harness.root.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));

    feed_command(&harness.first_model);
    feed_command(&harness.second_model);
    assert_eq!(harness.first_model.snapshot().command_marks().len(), 1);
    assert!(harness.first_model.command_marks_enabled());
    assert!(harness.second_model.command_marks_enabled());

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, COMMAND_MARKS_SWITCH_ID);
    click(harness.visual, super::settings_page::SETTINGS_SAVE_ID);
    wait_for_page_writes(&writes, 1);
    wait_for_page_idle(&mut harness, &root);

    assert!(!harness.first_model.command_marks_enabled());
    assert!(!harness.second_model.command_marks_enabled());
    assert!(
        harness.first_model.snapshot().command_marks().is_empty(),
        "the open pane drops the marks it had collected"
    );
    assert!(harness.second_model.snapshot().command_marks().is_empty());

    feed_command(&harness.first_model);
    assert!(
        harness.first_model.snapshot().command_marks().is_empty(),
        "the open pane stops collecting while the setting is off"
    );

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    let new_model = harness.visual.update(|_, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, test_shell(), &workspace.ui.locale, cx)
            });
            tab_binding(workspace, 3)
                .session()
                .expect("the new tab has a live session")
                .model()
        })
    });
    assert!(
        !new_model.command_marks_enabled(),
        "a new tab inherits the saved setting"
    );
    feed_command(&new_model);
    assert!(new_model.snapshot().command_marks().is_empty());

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_failed_save_keeps_the_marks_draft_and_retry_writes_it(cx: &mut TestAppContext) {
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
    let config = disabled_marks_config();
    let mut harness = harness_with_page_writer(cx, config.clone(), writer);
    let root = harness.root.clone();
    let state = harness.config.clone();
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(1200.0), gpui_kit::px(2400.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, COMMAND_MARKS_SWITCH_ID);
    let expected_draft = CommandMarksDraft {
        command_marks: true,
        ..marks_draft(&config)
    };

    click(harness.visual, super::settings_page::SETTINGS_SAVE_ID);
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.page_error(), Some("the config disk is full"));
        assert_eq!(
            page_marks_error(&root, cx).as_deref(),
            Some("the config disk is full")
        );
        assert_eq!(page_marks_values(&root, cx), expected_draft);
        assert!(!state.read(cx).config().command_marks);
    });
    assert_eq!(attempts.lock().expect("page attempts").len(), 1);

    click(harness.visual, super::settings_page::SETTINGS_SAVE_ID);
    wait_for_page_idle(&mut harness, &root);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(page.page_saved());
        assert!(page.page_error().is_none());
        assert!(page_marks_error(&root, cx).is_none());
        assert_eq!(page_marks_values(&root, cx), expected_draft);
        assert!(state.read(cx).config().command_marks);
    });
    assert_eq!(attempts.lock().expect("page attempts").len(), 2);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|_, cx| {
        assert_eq!(page_marks_values(&root, cx), expected_draft);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|window, _| window.remove_window());
}
