use gpui_kit::test::TestWindowExt as _;

use super::*;
use tterm_core::config::local_shells_file::ShellListEdit;
use tterm_core::connections::profile_recents::RecentProfile;
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::terminal::pty::TerminalShellProfile;
use tterm_core::workspace::local_shells::detected_shell_key;

use crate::terminal_demo::window::shell_launcher::recent::Fixture;

const RECENT_LABEL: &str = "Recent";
const EMPTY_RECENT_LABEL: &str = "No recent SSH connections yet.";
const RECENT_WAIT: Duration = Duration::from_secs(5);

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

/// Every label the open launcher lists, in order.
fn menu_labels(visual: &mut VisualTestContext) -> Vec<String> {
    let mut labels = Vec::new();
    for index in 0..64 {
        let Some(label) = menu_choice(visual, index) else {
            break;
        };
        labels.push(label);
    }
    labels
}

/// Asserts that Recent is one fixed submenu at the end of the launcher.
fn assert_recent_is_last(visual: &mut VisualTestContext) -> Vec<String> {
    let labels = menu_labels(visual);
    assert_eq!(
        labels.iter().filter(|label| *label == RECENT_LABEL).count(),
        1,
        "the launcher has exactly one Recent submenu: {labels:?}"
    );
    assert_eq!(
        labels.last().map(String::as_str),
        Some(RECENT_LABEL),
        "Recent follows every shell choice: {labels:?}"
    );
    labels
}

fn click_menu_index(visual: &mut VisualTestContext, index: usize) {
    visual.update(|window, cx| window.within("popup-menu").click(index, cx));
    visual.update(|window, cx| window.render_frame(cx));
}

fn open_recent_submenu(visual: &mut VisualTestContext) {
    let labels = assert_recent_is_last(visual);
    click_menu_index(visual, labels.len() - 1);
}

fn open_launcher_search(visual: &mut VisualTestContext) {
    click(visual, "new-shell-menu");
    let labels = menu_labels(visual);
    let index = labels
        .iter()
        .position(|label| label == "Search by name, host, username, or group")
        .unwrap();
    click_menu_index(visual, index);
    visual.run_until_parked();
}

fn launcher_labels(root: &Entity<Prototype>, visual: &mut VisualTestContext) -> Vec<String> {
    visual.update(|_, cx| {
        let picker = root.read(cx).picker.as_ref();
        let Some(super::picker::TabPicker::Launcher(panel)) = picker else {
            panic!("the launcher search is not open");
        };
        panel.read(cx).test_labels(cx)
    })
}

#[gpui_kit::test]
fn launcher_search_edits_filters_and_confirms_only_a_matching_id(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let visual = harness.visual;
    install_recent(
        visual,
        vec![ssh_summary("web", "Web"), ssh_summary("db", "Database")],
    );
    open_launcher_search(visual);
    visual.simulate_input("Datax");
    assert!(
        !launcher_labels(&harness.root, visual)
            .iter()
            .any(|row| row.starts_with("Database ·"))
    );
    press(visual, "backspace");
    click(visual, "launcher-row-0");
    wait_for_confirmed(visual, "db");
    install_recent(
        visual,
        vec![ssh_summary("web", "Web"), ssh_summary("db", "Database")],
    );
    open_launcher_search(visual);
    visual.simulate_input("Database");
    press(visual, &format!("{}-a backspace", modifier()));
    assert!(
        launcher_labels(&harness.root, visual)
            .iter()
            .any(|row| row.starts_with("Web ·"))
    );
    press(visual, "escape");
    assert_eq!(confirmed_recent(visual), None);
    open_launcher_search(visual);
    visual.simulate_input("Data");
    press(visual, "enter");
    wait_for_confirmed(visual, "db");
    install_recent(visual, vec![ssh_summary("one", "One")]);
    open_launcher_search(visual);
    visual.simulate_input("deploy");
    press(visual, "escape");
    assert_eq!(confirmed_recent(visual), None);
    visual.update(|window, cx| {
        let workspace = harness.root.read(cx);
        assert!(workspace.picker.is_none());
        assert!(
            workspace
                .tabs
                .active()
                .view
                .read(cx)
                .focus_handle()
                .is_focused(window)
        );
        window.remove_window();
    });
}

#[gpui_kit::test]
fn launcher_search_starts_with_shells_new_connection_and_recent_profiles(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let visual = harness.visual;
    visual.update(|_, cx| {
        Fixture::install_with_recents(
            cx,
            vec![ssh_summary("web", "Web")],
            vec![RecentProfile {
                profile_id: "web".into(),
                last_used_at: 1,
            }],
        );
    });
    open_launcher_search(visual);
    let labels = launcher_labels(&root, visual);
    assert!(
        labels.iter().any(|label| label.contains("New connection")),
        "the launcher offers a new connection: {labels:?}"
    );
    assert_eq!(
        labels.last().map(String::as_str),
        Some("Web · deploy@10.0.0.5:2222"),
        "the recently used saved connection comes after the default actions: {labels:?}"
    );
    press(visual, "escape");
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn launcher_search_arrows_confirm_one_of_two_same_named_profiles(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let visual = harness.visual;
    install_recent(
        visual,
        vec![ssh_summary("first", "Twin"), ssh_summary("second", "Twin")],
    );
    open_launcher_search(visual);
    visual.simulate_input("Twin");
    press(visual, "down up down enter");
    wait_for_confirmed(visual, "second");
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn launcher_add_user_caret_and_enter_open_one_unsaved_ssh_tab(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let visual = harness.visual;
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    open_launcher_search(visual);
    visual.simulate_input("tester@127.0.0.1:65536");
    assert!(
        !launcher_labels(&harness.root, visual)
            .iter()
            .any(|row| row.starts_with("Connect to"))
    );
    press(visual, &format!("{}-a backspace", modifier()));
    let endpoint = format!("127.0.0.1:{port}");
    visual.simulate_input(&endpoint);
    press(visual, "enter");
    visual.simulate_input("tester");
    assert!(launcher_labels(&harness.root, visual)[0].contains(&format!("tester@{endpoint}")));
    press(visual, "enter");
    visual.update(|window, cx| {
        let workspace = harness.root.read(cx);
        assert!(workspace.picker.is_none());
        assert_eq!(workspace.tabs.len(), 3);
        let tab = workspace.tabs.active().saved();
        assert_eq!(tab.quick_connect, Some(true));
        let connection = tab.connection.as_ref().unwrap();
        assert_eq!(connection.host.as_deref(), Some("127.0.0.1"));
        assert_eq!(connection.port, Some(u32::from(port)));
        assert_eq!(connection.username.as_deref(), Some("tester"));
        assert!(connection.profile_id.is_none() && connection.password.is_none());
        window.remove_window();
    });
}

#[gpui_kit::test]
fn launcher_search_filters_local_shells_by_their_label(cx: &mut TestAppContext) {
    let harness = harness(
        cx,
        AppConfig {
            local_shells: vec![LocalShell {
                id: "picked".into(),
                name: "Picked shell".into(),
                path: "/bin/sh".into(),
                args: String::new(),
            }],
            ..Default::default()
        },
    );
    let root = harness.root.clone();
    let visual = harness.visual;
    open_launcher_search(visual);
    visual.simulate_input("Picked");
    let labels = launcher_labels(&root, visual);
    assert!(
        labels
            .iter()
            .any(|label| label.starts_with("Picked shell ·")),
        "the typed label keeps its local shell: {labels:?}"
    );
    press(visual, "escape");
    visual.update(|window, _| window.remove_window());
}

fn submenu_choice(visual: &mut VisualTestContext, index: usize) -> Option<String> {
    visual.update(|window, _| {
        window.try_find("submenu")?;
        window
            .within("submenu")
            .try_find(index)
            .and_then(|item| item.label().map(str::to_string))
    })
}

fn wait_for_first_recent(visual: &mut VisualTestContext, expected: &str) {
    let deadline = Instant::now() + RECENT_WAIT;
    loop {
        visual.run_until_parked();
        if submenu_choice(visual, 0).as_deref() == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Recent never listed {expected:?}; it lists {:?}",
            submenu_choice(visual, 0)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn install_recent(visual: &mut VisualTestContext, profiles: Vec<SavedSshProfileSummary>) {
    visual.update(|_, cx| Fixture::install(cx, profiles));
}

fn confirmed_recent(visual: &mut VisualTestContext) -> Option<String> {
    visual.update(|_, cx| Fixture::confirmed(cx))
}

fn wait_for_confirmed(visual: &mut VisualTestContext, expected: &str) {
    let deadline = Instant::now() + RECENT_WAIT;
    loop {
        visual.run_until_parked();
        if confirmed_recent(visual).as_deref() == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Recent confirmed {:?} instead of {expected:?}",
            confirmed_recent(visual)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn ssh_summary(id: &str, name: &str) -> SavedSshProfileSummary {
    SavedSshProfileSummary {
        id: id.to_string(),
        name: name.to_string(),
        username: Some("deploy".to_string()),
        host: Some("10.0.0.5".to_string()),
        port: Some(2222),
        group_path: Some("Production".to_string()),
    }
}

fn recent_item_label(profile: &SavedSshProfileSummary) -> String {
    format!("{} · {}", profile.name, profile.endpoint())
}

fn click_submenu_index(visual: &mut VisualTestContext, index: usize) {
    visual.update(|window, cx| window.within("submenu").click(index, cx));
    visual.update(|window, cx| window.render_frame(cx));
}

fn close_open_menus(visual: &mut VisualTestContext) {
    for _ in 0..3 {
        let open = visual.update(|window, _| {
            !window.find_all("popup-menu").is_empty() || window.try_find("submenu").is_some()
        });
        if !open {
            return;
        }
        press(visual, "escape");
    }
    panic!("the launcher did not close after Escape");
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
    let labels = assert_recent_is_last(harness.visual);
    assert!(
        !labels.iter().any(|label| label == "Debian"),
        "a hidden non-default distribution is not listed: {labels:?}"
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
    let labels = assert_recent_is_last(harness.visual);
    assert!(
        !labels.iter().any(|label| label == "Detected shell"),
        "a hidden detected shell is not listed: {labels:?}"
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
    assert_recent_is_last(harness.visual);
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
    assert_recent_is_last(harness.visual);
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
    let labels = assert_recent_is_last(harness.visual);
    assert!(
        !labels.iter().any(|label| label == "Default custom"),
        "the removed shell is not listed: {labels:?}"
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

#[gpui_kit::test]
fn recent_submenu_is_last_and_shows_its_empty_message(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first = Rc::clone(&harness.first);
    install_recent(harness.visual, Vec::new());
    assert_eq!(
        confirmed_recent(harness.visual),
        None,
        "a new fixture starts without a confirmed recent"
    );

    click(harness.visual, "new-shell-menu");
    let labels = assert_recent_is_last(harness.visual);
    assert!(
        labels.len() > 1,
        "the shells stay listed ahead of Recent: {labels:?}"
    );
    open_recent_submenu(harness.visual);
    wait_for_first_recent(harness.visual, EMPTY_RECENT_LABEL);
    assert_eq!(
        submenu_choice(harness.visual, 1),
        None,
        "an empty fixture lists only the not-found message"
    );
    assert_eq!(confirmed_recent(harness.visual), None);

    close_open_menus(harness.visual);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 2);
        assert_eq!(workspace.tabs.active_id(), 1);
        assert!(Rc::ptr_eq(&first, &tab_binding(workspace, 1)));
        assert!(
            workspace
                .tabs
                .active()
                .view
                .read(cx)
                .focus_handle()
                .is_focused(window),
            "closing the empty Recent submenu returns focus to the open tab"
        );
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn clicking_a_recent_ssh_profile_confirms_its_id_without_opening_a_tab(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first = Rc::clone(&harness.first);
    let profile = ssh_summary("prod-web", "Production web");
    install_recent(harness.visual, vec![profile.clone()]);

    click(harness.visual, "new-shell-menu");
    open_recent_submenu(harness.visual);
    let label = recent_item_label(&profile);
    wait_for_first_recent(harness.visual, &label);
    click_submenu_index(harness.visual, 0);
    wait_for_confirmed(harness.visual, &profile.id);

    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 2, "the test double does not dial");
        assert_eq!(workspace.tabs.active_id(), 1);
        assert!(Rc::ptr_eq(&first, &tab_binding(workspace, 1)));
        assert!(window.try_find("popup-menu").is_none());
        assert!(window.try_find("submenu").is_none());
    });

    click(harness.visual, "new-shell-menu");
    close_open_menus(harness.visual);
    harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 2);
        assert_eq!(workspace.tabs.active_id(), 1);
        assert!(Rc::ptr_eq(&first, &tab_binding(workspace, 1)));
    });
    assert_eq!(
        confirmed_recent(harness.visual).as_deref(),
        Some("prod-web"),
        "Esc does not replace the stable id already confirmed"
    );
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn keyboard_selection_of_a_recent_ssh_profile_confirms_it(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first = Rc::clone(&harness.first);
    let profile = ssh_summary("prod-db", "Production db");
    install_recent(harness.visual, vec![profile.clone()]);

    click(harness.visual, "new-shell-menu");
    assert_recent_is_last(harness.visual);
    for _ in 0..64 {
        if harness
            .visual
            .update(|window, _| window.try_find("submenu").is_some())
        {
            break;
        }
        press(harness.visual, "down");
    }
    assert!(
        harness
            .visual
            .update(|window, _| window.try_find("submenu").is_some()),
        "the keyboard never opened the Recent submenu"
    );
    wait_for_first_recent(harness.visual, &recent_item_label(&profile));
    // Opening the submenu leaves focus in the parent popup, so the first Enter
    // enters the submenu and highlights its first item; the second activates it.
    press(harness.visual, "enter enter");
    wait_for_confirmed(harness.visual, &profile.id);

    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 2);
        assert_eq!(workspace.tabs.active_id(), 1);
        assert!(Rc::ptr_eq(&first, &tab_binding(workspace, 1)));
        assert!(window.try_find("popup-menu").is_none());
        assert!(window.try_find("submenu").is_none());
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn opening_recent_again_loads_the_current_fixture(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let first = ssh_summary("old-web", "Old web");
    let second = ssh_summary("new-db", "New db");
    install_recent(harness.visual, vec![first.clone()]);

    click(harness.visual, "new-shell-menu");
    open_recent_submenu(harness.visual);
    wait_for_first_recent(harness.visual, &recent_item_label(&first));
    close_open_menus(harness.visual);

    install_recent(harness.visual, vec![second.clone()]);
    click(harness.visual, "new-shell-menu");
    open_recent_submenu(harness.visual);
    wait_for_first_recent(harness.visual, &recent_item_label(&second));
    assert_eq!(
        submenu_choice(harness.visual, 1),
        None,
        "the refreshed fixture replaces the first list"
    );
    assert_eq!(
        confirmed_recent(harness.visual),
        None,
        "refreshing the list does not confirm a connection"
    );
    close_open_menus(harness.visual);
    harness.visual.update(|window, _| window.remove_window());
}
