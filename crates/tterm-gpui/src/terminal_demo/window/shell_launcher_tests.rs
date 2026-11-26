use gpui_kit::test::TestWindowExt as _;

use super::*;
use tterm_core::config::local_shells_file::ShellListEdit;
use tterm_core::terminal::pty::TerminalShellProfile;
use tterm_core::workspace::local_shells::detected_shell_key;

/// Applies one shell-list edit and waits for the configuration to accept it.
async fn edit_shell_list(harness: &mut Harness<'_>, edit: ShellListEdit) {
    let edit = harness.visual.update(|_, cx| {
        harness
            .config
            .update(cx, |state, cx| state.edit_shell_list(edit, cx))
    });
    assert!(edit.await.is_ok(), "the shell list edit should be accepted");
}

/// The accessible label of the open launcher's choice at `index`.
fn menu_choice(visual: &mut VisualTestContext, index: usize) -> Option<String> {
    visual.update(|window, _| {
        window
            .within("popup-menu")
            .try_find(index)
            .and_then(|choice| choice.label().map(str::to_string))
    })
}

/// The checked state of the open launcher's choice at `index`.
fn menu_checked(visual: &mut VisualTestContext, index: usize) -> Option<bool> {
    visual.update(|window, _| {
        window
            .within("popup-menu")
            .try_find(index)
            .and_then(|choice| choice.checked())
    })
}

#[gpui_kit::test]
fn local_shell_menu_cancels_then_launches_the_saved_program(cx: &mut TestAppContext) {
    #[cfg(unix)]
    let (path, args) = ("/bin/sh", "-c \"echo PICKED_PROGRAM; read answer\"");
    #[cfg(windows)]
    let (path, args) = ("cmd.exe", "/d /q /k echo PICKED_PROGRAM");
    let harness = harness(
        cx,
        AppConfig {
            local_shells: vec![LocalShell {
                id: "picked".into(),
                name: "Picked".into(),
                path: path.into(),
                args: args.into(),
            }],
            ..Default::default()
        },
    );
    let root = harness.root.clone();
    click(harness.visual, "new-shell-menu");
    press(harness.visual, "escape");
    harness.visual.update(|window, cx| {
        assert_eq!(root.read(cx).tabs.len(), 2);
        assert_eq!(root.read(cx).tabs.active_id(), 1);
        assert!(
            root.read(cx)
                .tabs
                .active()
                .view
                .read(cx)
                .focus_handle()
                .is_focused(window)
        );
    });
    click(harness.visual, "new-shell-menu");
    press(harness.visual, "down down enter");
    let model = harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 3);
        assert!(Rc::ptr_eq(&harness.first, &tab_binding(workspace, 1)));
        let binding = tab_binding(workspace, workspace.tabs.active_id());
        binding.session().expect("selected local PTY").model()
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !output(&model).contains("PICKED_PROGRAM") {
        assert!(
            Instant::now() < deadline,
            "selected program output: {}",
            output(&model)
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn menu_lists_synthetic_detected_shells_and_uses_the_selected_choice(cx: &mut TestAppContext) {
    #[cfg(unix)]
    let detected = "/bin/sh";
    #[cfg(windows)]
    let detected = "cmd.exe";
    let harness = harness(cx, AppConfig::default());
    harness.visual.update(|_, app| {
        harness.config.update(app, |state, _| {
            state.set_detected_shells_for_test(vec![TerminalShellProfile {
                shell: "custom".into(),
                label: "Detected shell".into(),
                source: detected.into(),
                ..Default::default()
            }]);
        });
    });
    let root = harness.root.clone();
    click(harness.visual, "new-shell-menu");
    press(harness.visual, "down down enter");
    harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 3);
        assert!(Rc::ptr_eq(&harness.first, &tab_binding(workspace, 1)));
        let connection = workspace
            .tabs
            .active()
            .saved()
            .connection
            .as_ref()
            .expect("selected local shell connection");
        assert_eq!(
            connection.saved_field("terminalShell"),
            Some(&serde_json::json!("custom"))
        );
        assert_eq!(
            connection.saved_field("terminalShellCustomPath"),
            Some(&serde_json::json!(detected))
        );
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn configured_wsl_distro_is_listed_first_checked_and_keeps_hidden_entries_out(
    cx: &mut TestAppContext,
) {
    let harness = harness(
        cx,
        AppConfig {
            terminal_shell: "wsl".into(),
            terminal_shell_wsl_distro: "Ubuntu".into(),
            hidden_shells: vec!["wsl:Ubuntu".into(), "wsl:Debian".into()],
            ..Default::default()
        },
    );
    harness.visual.update(|_, app| {
        harness.config.update(app, |state, _| {
            state.set_detected_shells_for_test(vec![
                TerminalShellProfile {
                    shell: "wsl".into(),
                    label: "Ubuntu".into(),
                    source: "wsl.exe".into(),
                    wsl_distro: Some("Ubuntu".into()),
                    default_wsl_distro: true,
                },
                TerminalShellProfile {
                    shell: "wsl".into(),
                    label: "Debian".into(),
                    source: "wsl.exe".into(),
                    wsl_distro: Some("Debian".into()),
                    ..Default::default()
                },
            ]);
        });
    });
    let root = harness.root.clone();

    click(harness.visual, "new-shell-menu");
    assert_eq!(menu_choice(harness.visual, 0).as_deref(), Some("Ubuntu"));
    assert_eq!(menu_checked(harness.visual, 0), Some(true));
    assert!(
        menu_choice(harness.visual, 2).is_none(),
        "a hidden non-default distribution is not listed"
    );

    press(harness.visual, "escape");
    harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 2);
        assert_eq!(workspace.tabs.active_id(), 1);
        assert!(Rc::ptr_eq(&harness.first, &tab_binding(workspace, 1)));
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn configured_default_without_an_entry_keeps_its_action(cx: &mut TestAppContext) {
    let path = if cfg!(windows) {
        r"C:\missing\shell.exe"
    } else {
        "/missing/shell"
    };
    let harness = harness(
        cx,
        AppConfig {
            terminal_shell: "custom".into(),
            terminal_shell_custom_path: path.into(),
            terminal_shell_custom_args: "-l".into(),
            ..Default::default()
        },
    );
    let root = harness.root.clone();

    click(harness.visual, "new-shell-menu");
    assert_eq!(
        menu_choice(harness.visual, 0).as_deref(),
        Some(path),
        "the saved path names the default even without a list entry"
    );
    assert_eq!(menu_checked(harness.visual, 0), Some(true));
    assert_eq!(
        menu_checked(harness.visual, 1),
        None,
        "Auto is not marked as the default"
    );

    press(harness.visual, "escape");
    harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 2);
        assert_eq!(workspace.tabs.active_id(), 1);
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn explicit_auto_starts_auto_instead_of_the_configured_default(cx: &mut TestAppContext) {
    #[cfg(unix)]
    let (path, args) = ("/bin/sh", "-c \"echo DEFAULT_SHELL; read answer\"");
    #[cfg(windows)]
    let (path, args) = ("cmd.exe", "/d /q /k echo DEFAULT_SHELL");
    let harness = harness(
        cx,
        AppConfig {
            terminal_shell: "custom".into(),
            terminal_shell_custom_path: path.into(),
            terminal_shell_custom_args: args.into(),
            ..Default::default()
        },
    );
    let root = harness.root.clone();
    click(harness.visual, "new-shell-menu");
    press(harness.visual, "down down enter");
    harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 3);
        assert!(Rc::ptr_eq(&harness.first, &tab_binding(workspace, 1)));
        let connection = workspace
            .tabs
            .active()
            .saved()
            .connection
            .as_ref()
            .expect("selected local shell connection");
        assert_eq!(
            connection.saved_field("terminalShell"),
            Some(&serde_json::json!("auto"))
        );
        assert_eq!(connection.saved_field("terminalShellCustomPath"), None);
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn ordinary_new_tab_starts_the_configured_custom_default(cx: &mut TestAppContext) {
    #[cfg(unix)]
    let (path, args, marker) = (
        "/bin/sh",
        "-c \"echo DEFAULT_CUSTOM_MARKER; read answer\"",
        "DEFAULT_CUSTOM_MARKER",
    );
    #[cfg(windows)]
    let (path, args, marker) = (
        "cmd.exe",
        "/d /q /k echo DEFAULT_CUSTOM_MARKER",
        "DEFAULT_CUSTOM_MARKER",
    );
    let shell = LocalShell {
        id: "default-custom".into(),
        name: "Default custom".into(),
        path: path.into(),
        args: args.into(),
    };
    let harness = harness(
        cx,
        AppConfig {
            terminal_shell: "custom".into(),
            terminal_shell_id: shell.id.clone(),
            terminal_shell_custom_path: "/stale/path".into(),
            terminal_shell_custom_args: "--stale".into(),
            local_shells: vec![shell],
            ..Default::default()
        },
    );
    let root = harness.root.clone();
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| workspace.new_tab(window, cx));
    });
    let model = harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 3);
        assert!(Rc::ptr_eq(&harness.first, &tab_binding(workspace, 1)));
        tab_binding(workspace, workspace.tabs.active_id())
            .session()
            .expect("selected local PTY")
            .model()
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !output(&model).contains(marker) {
        assert!(
            Instant::now() < deadline,
            "default custom shell output: {}",
            output(&model)
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn menu_starts_the_selected_custom_shell_rather_than_the_default(cx: &mut TestAppContext) {
    #[cfg(unix)]
    let (path, args) = ("/bin/sh", "-c \"echo SELECTED_SHELL; read answer\"");
    #[cfg(windows)]
    let (path, args) = ("cmd.exe", "/d /q /k echo SELECTED_SHELL");
    let default = LocalShell {
        id: "default".into(),
        name: "Default".into(),
        path: path.into(),
        args: args.into(),
    };
    let selected = LocalShell {
        id: "selected".into(),
        name: "Selected".into(),
        path: path.into(),
        args: args.into(),
    };
    let harness = harness(
        cx,
        AppConfig {
            terminal_shell: "custom".into(),
            terminal_shell_id: default.id.clone(),
            terminal_shell_custom_path: default.path.clone(),
            terminal_shell_custom_args: default.args.clone(),
            local_shells: vec![default, selected.clone()],
            ..Default::default()
        },
    );
    let root = harness.root.clone();
    click(harness.visual, "new-shell-menu");
    press(harness.visual, "down down down enter");
    harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 3);
        assert!(Rc::ptr_eq(&harness.first, &tab_binding(workspace, 1)));
        let connection = workspace
            .tabs
            .active()
            .saved()
            .connection
            .as_ref()
            .expect("selected local shell connection");
        assert_eq!(
            connection.saved_field("terminalShellId"),
            Some(&serde_json::json!(selected.id))
        );
        assert_eq!(
            connection.saved_field("terminalShellCustomArgs"),
            Some(&serde_json::json!(selected.args))
        );
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
async fn hiding_and_showing_a_detected_shell_follows_the_launcher(cx: &mut TestAppContext) {
    #[cfg(unix)]
    let detected = "/bin/sh";
    #[cfg(windows)]
    let detected = "cmd.exe";
    let mut harness = harness(cx, AppConfig::default());
    let profile = TerminalShellProfile {
        shell: "custom".into(),
        label: "Detected shell".into(),
        source: detected.into(),
        ..Default::default()
    };
    harness.visual.update(|_, app| {
        harness.config.update(app, |state, _| {
            state.set_detected_shells_for_test(vec![profile])
        });
    });
    let key = detected_shell_key(&TerminalShellProfile {
        shell: "custom".into(),
        source: detected.into(),
        ..Default::default()
    });
    let root = harness.root.clone();

    edit_shell_list(
        &mut harness,
        ShellListEdit::Visibility {
            key: key.clone(),
            hidden: true,
        },
    )
    .await;
    click(harness.visual, "new-shell-menu");
    assert!(
        menu_choice(harness.visual, 0).is_some(),
        "Auto is always listed"
    );
    assert!(
        menu_choice(harness.visual, 1).is_none(),
        "a hidden detected shell is not listed"
    );
    press(harness.visual, "down enter");
    harness.visual.update(|_, cx| {
        let connection = root
            .read(cx)
            .tabs
            .active()
            .saved()
            .connection
            .as_ref()
            .expect("the new tab connection");
        assert_eq!(
            connection.saved_field("terminalShell"),
            Some(&serde_json::json!("auto")),
            "the only listed choice is Auto"
        );
    });

    edit_shell_list(
        &mut harness,
        ShellListEdit::Visibility { key, hidden: false },
    )
    .await;
    click(harness.visual, "new-shell-menu");
    assert_eq!(
        menu_choice(harness.visual, 1).as_deref(),
        Some("Detected shell"),
        "showing the shell lists it again"
    );
    press(harness.visual, "down down enter");
    harness.visual.update(|_, cx| {
        let connection = root
            .read(cx)
            .tabs
            .active()
            .saved()
            .connection
            .as_ref()
            .expect("the new tab connection");
        assert_eq!(
            connection.saved_field("terminalShell"),
            Some(&serde_json::json!("custom"))
        );
        assert_eq!(
            connection.saved_field("terminalShellCustomPath"),
            Some(&serde_json::json!(detected))
        );
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
async fn moving_a_custom_shell_changes_which_menu_choice_opens(cx: &mut TestAppContext) {
    #[cfg(unix)]
    let (path, args) = ("/bin/sh", "-c \"echo MOVED_SHELL; read answer\"");
    #[cfg(windows)]
    let (path, args) = ("cmd.exe", "/d /q /k echo MOVED_SHELL");
    let first = LocalShell {
        id: "first".into(),
        name: "First".into(),
        path: path.into(),
        args: args.into(),
    };
    let second = LocalShell {
        id: "second".into(),
        name: "Second".into(),
        path: path.into(),
        args: args.into(),
    };
    let mut harness = harness(
        cx,
        AppConfig {
            local_shells: vec![first, second.clone()],
            ..Default::default()
        },
    );
    let root = harness.root.clone();

    edit_shell_list(
        &mut harness,
        ShellListEdit::Move {
            id: second.id.clone(),
            offset: -1,
        },
    )
    .await;
    click(harness.visual, "new-shell-menu");
    assert_eq!(
        menu_choice(harness.visual, 1).as_deref(),
        Some("Second"),
        "the moved shell is listed first among the user's shells"
    );
    assert_eq!(menu_choice(harness.visual, 2).as_deref(), Some("First"));
    press(harness.visual, "down down enter");
    harness.visual.update(|_, cx| {
        let connection = root
            .read(cx)
            .tabs
            .active()
            .saved()
            .connection
            .as_ref()
            .expect("the new tab connection");
        assert_eq!(
            connection.saved_field("terminalShellId"),
            Some(&serde_json::json!(second.id)),
            "the same keystrokes start the shell the menu now lists there"
        );
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
async fn removing_the_active_custom_default_starts_auto_and_keeps_the_open_session(
    cx: &mut TestAppContext,
) {
    #[cfg(unix)]
    let (path, args, marker) = (
        "/bin/sh",
        "-c \"echo REMOVED_DEFAULT; read answer\"",
        "REMOVED_DEFAULT",
    );
    #[cfg(windows)]
    let (path, args, marker) = (
        "cmd.exe",
        "/d /q /k echo REMOVED_DEFAULT",
        "REMOVED_DEFAULT",
    );
    let shell = LocalShell {
        id: "default-custom".into(),
        name: "Default custom".into(),
        path: path.into(),
        args: args.into(),
    };
    let mut harness = harness(
        cx,
        AppConfig {
            terminal_shell: "custom".into(),
            terminal_shell_id: shell.id.clone(),
            terminal_shell_custom_path: path.into(),
            terminal_shell_custom_args: args.into(),
            local_shells: vec![shell.clone()],
            ..Default::default()
        },
    );
    let root = harness.root.clone();
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| workspace.new_tab(window, cx));
    });
    let (opened, binding, model) = harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        let opened = workspace.tabs.active_id();
        let binding = tab_binding(workspace, opened);
        let model = binding.session().expect("the custom default PTY").model();
        (opened, binding, model)
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !output(&model).contains(marker) {
        assert!(
            Instant::now() < deadline,
            "default custom shell output: {}",
            output(&model)
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }

    edit_shell_list(
        &mut harness,
        ShellListEdit::Remove {
            id: shell.id.clone(),
        },
    )
    .await;

    let still_open = harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        let binding_now = tab_binding(workspace, opened);
        assert!(
            Rc::ptr_eq(&binding, &binding_now),
            "the open tab keeps its binding"
        );
        binding_now.session().expect("the open PTY").model()
    });
    assert!(
        Arc::ptr_eq(&model, &still_open),
        "removing the default shell does not restart the open tab"
    );
    assert!(output(&still_open).contains(marker));
    click(harness.visual, "new-shell-menu");
    assert!(
        menu_choice(harness.visual, 1).is_none(),
        "the removed shell is not listed"
    );
    press(harness.visual, "escape");

    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| workspace.new_tab(window, cx));
    });
    harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 4);
        let connection = workspace
            .tabs
            .active()
            .saved()
            .connection
            .as_ref()
            .expect("the new tab connection");
        assert_eq!(
            connection.saved_field("terminalShell"),
            Some(&serde_json::json!("auto")),
            "a removed default falls back to Auto"
        );
        assert_eq!(connection.saved_field("terminalShellCustomPath"), None);
    });
    harness.visual.update(|window, _| window.remove_window());
}
