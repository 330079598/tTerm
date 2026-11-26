//! Regressions for the saved SSH connection editor over a live workspace.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{App, Entity, MouseButton, TestAppContext, VisualTestContext};
use tterm_core::config::AppConfig;
use tterm_core::profiles::launch::SavedSshProfileSummary;

use super::super::profile::{AuthKind, ProfileDraft};
use super::super::{
    EDIT_BUTTON_ID, EDITOR_AUTH_KEY_ID, EDITOR_CANCEL_ID, EDITOR_KEY_PATH_ID, EDITOR_SAVE_ID,
    EMPTY_NEW_BUTTON_ID, NEW_BUTTON_ID, OPEN_BUTTON_ID, SavedConnectionsUi,
};
use super::{
    EDITOR_GROUP_ID, EditorField, EditorValues, build_draft, group_path_changed, invalid_field,
};
use crate::terminal_demo::command::CommandSpec;
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};
use crate::terminal_demo::window::Prototype;
use crate::terminal_demo::window::split_session_tests::make_workspace_with;

struct Harness<'a> {
    _runtime: GpuiRuntime,
    root: Entity<Prototype>,
    visual: &'a mut VisualTestContext,
}

const SECOND_ROW: &str = "saved-connections-row-1";

fn shell() -> CommandSpec {
    #[cfg(windows)]
    let (program, args) = ("cmd.exe", vec!["/d".into(), "/q".into()]);
    #[cfg(not(windows))]
    let (program, args) = ("/bin/sh", vec!["-i".into()]);
    CommandSpec::Program {
        program: program.into(),
        args,
    }
}

fn harness<'a>(cx: &'a mut TestAppContext) -> Harness<'a> {
    let mut runtime = test_runtime();
    let events = std::mem::replace(
        &mut runtime.events,
        tokio::sync::mpsc::unbounded_channel().1,
    );
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
    });
    let (root, visual) = cx.add_window_view(|_, cx| {
        make_workspace_with(&runtime, events, shell(), AppConfig::default(), cx)
    });
    Harness {
        _runtime: runtime,
        root,
        visual,
    }
}

fn summary(id: &str, name: &str, port: u16) -> SavedSshProfileSummary {
    SavedSshProfileSummary {
        id: id.to_string(),
        name: name.to_string(),
        username: Some("deploy".to_string()),
        host: Some("host.example".to_string()),
        port: Some(port),
        group_path: None,
    }
}

fn picker<'a>(root: &'a Entity<Prototype>, cx: &'a App) -> &'a SavedConnectionsUi {
    root.read(cx).ui.saved_connections.read(cx)
}

fn with_picker(
    root: &Entity<Prototype>,
    cx: &mut App,
    update: impl FnOnce(&mut SavedConnectionsUi, &mut gpui_kit::Context<SavedConnectionsUi>),
) {
    let picker = root.read(cx).ui.saved_connections.clone();
    picker.update(cx, update);
}

fn click(visual: &mut VisualTestContext, selector: &'static str) {
    visual.update(|window, cx| window.render_frame(cx));
    let bounds = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} should be laid out"));
    let center = bounds.center();
    visual.simulate_mouse_down(center, MouseButton::Left, Default::default());
    visual.simulate_mouse_up(center, MouseButton::Left, Default::default());
    visual.update(|window, cx| window.render_frame(cx));
}

fn open_picker(harness: &mut Harness<'_>) {
    click(harness.visual, OPEN_BUTTON_ID);
    harness.visual.update(|window, cx| window.render_frame(cx));
}

fn focus_field(harness: &mut Harness<'_>, field: EditorField) {
    let root = harness.root.clone();
    harness.visual.update(|window, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.focus_editor_field_for_test(field, window, cx);
        });
    });
}

#[gpui_kit::test]
fn an_empty_library_creates_selects_and_cancels_a_connection(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(Vec::new()), cx);
            picker.set_editor_save_for_test(Ok("saved-new".to_string()));
        });
    });
    open_picker(&mut harness);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(picker.is_open());
        assert!(picker.profiles().is_empty());
    });
    assert!(
        harness.visual.debug_bounds(EMPTY_NEW_BUTTON_ID).is_some(),
        "the empty list should offer New Connection"
    );
    click(harness.visual, EMPTY_NEW_BUTTON_ID);
    harness.visual.update(|window, cx| {
        let picker = picker(&root, cx);
        assert!(picker.editor_is_open());
        assert!(picker.editor_name_focused_for_test(window, cx));
        let values = picker.editor_values_for_test(cx).expect("editor values");
        assert_eq!(values.port, "22", "a new connection starts on port 22");
    });
    assert!(
        harness.visual.debug_bounds(EDITOR_GROUP_ID).is_some(),
        "the group path field should be laid out"
    );
    harness.visual.simulate_input("Prod Box");
    focus_field(&mut harness, EditorField::Group);
    harness.visual.simulate_input("Prod / Web");
    focus_field(&mut harness, EditorField::Host);
    harness.visual.simulate_input("prod.example.com");
    focus_field(&mut harness, EditorField::Username);
    harness.visual.simulate_input("deploy");
    harness.visual.update(|window, cx| window.render_frame(cx));
    assert!(
        harness.visual.debug_bounds(EDITOR_KEY_PATH_ID).is_none(),
        "password sign-in must not show a key path field"
    );
    click(harness.visual, EDITOR_AUTH_KEY_ID);
    assert!(
        harness.visual.debug_bounds(EDITOR_KEY_PATH_ID).is_some(),
        "the key path appears only for key-file sign-in"
    );
    focus_field(&mut harness, EditorField::KeyPath);
    harness.visual.simulate_input("~/.ssh/id_ed25519");
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        let values = picker.editor_values_for_test(cx).expect("editor values");
        assert_eq!(values.name, "Prod Box");
        assert_eq!(values.host, "prod.example.com");
        assert_eq!(values.username, "deploy");
        assert!(values.key_auth);
        assert_eq!(values.key_path, "~/.ssh/id_ed25519");
        assert_eq!(values.group_path, "Prod / Web");
        assert!(values.group_changed);
    });
    // The refresh after a save reads the real list asynchronously. Feed it
    // the saved summary before the save finishes so the new id can be found.
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(vec![summary("saved-new", "Prod Box", 22)]), cx);
        });
    });
    // Enter is the editor's save key; it must not fall through to the list's
    // Enter-to-open behavior while the form is open.
    harness.visual.simulate_keystrokes("enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(
            !picker.editor_is_open(),
            "a successful save returns to the list"
        );
        assert!(picker.is_open());
        assert_eq!(picker.profiles().len(), 1);
        assert_eq!(picker.selected_profile_id(), Some("saved-new"));
        assert!(!picker.is_opening());
    });
    click(harness.visual, NEW_BUTTON_ID);
    harness.visual.update(|window, cx| {
        assert!(picker(&root, cx).editor_is_open());
        assert!(picker(&root, cx).editor_name_focused_for_test(window, cx));
    });
    harness.visual.simulate_input("Discard me");
    click(harness.visual, EDITOR_CANCEL_ID);
    harness.visual.update(|window, cx| {
        let picker = picker(&root, cx);
        assert!(!picker.editor_is_open(), "Cancel must return to the list");
        assert!(picker.is_open());
        assert_eq!(picker.selected_profile_id(), Some("saved-new"));
        assert!(picker.list_focus_for_test().is_focused(window));
    });
    click(harness.visual, NEW_BUTTON_ID);
    harness.visual.update(|window, cx| {
        assert!(picker(&root, cx).editor_is_open());
        assert!(picker(&root, cx).editor_name_focused_for_test(window, cx));
    });
    harness.visual.simulate_keystrokes("escape");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(!picker.editor_is_open(), "Escape must return to the list");
        assert!(picker.is_open());
        assert_eq!(picker.selected_profile_id(), Some("saved-new"));
        assert!(picker.list_focus_for_test().is_focused(window));
    });
    harness.visual.simulate_keystrokes("escape");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(
            !picker(&root, cx).is_open(),
            "Escape in the list leaves the picker"
        );
    });
}

#[gpui_kit::test]
fn editing_keeps_the_id_and_unrelated_fields(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(
                Ok(vec![
                    summary("alpha", "Alpha", 22),
                    summary("edit-me", "Edit Me", 2222),
                ]),
                cx,
            );
            picker.set_editor_load_for_test(Ok(ProfileDraft {
                id: Some("edit-me".to_string()),
                name: "Edit Me".to_string(),
                host: "old.example".to_string(),
                port: "2222".to_string(),
                username: "alice".to_string(),
                auth: AuthKind::Key,
                key_path: "~/.ssh/id_ed25519".to_string(),
                group_path: "Prod / Web".to_string(),
                group_changed: false,
            }));
            picker.set_editor_save_for_test(Ok("edit-me".to_string()));
        });
    });
    open_picker(&mut harness);
    click(harness.visual, SECOND_ROW);
    click(harness.visual, EDIT_BUTTON_ID);
    harness.visual.update(|window, cx| {
        let picker = picker(&root, cx);
        assert!(picker.editor_is_open());
        assert!(picker.editor_name_focused_for_test(window, cx));
        assert_eq!(picker.editor_id_for_test(), Some("edit-me"));
        let values = picker.editor_values_for_test(cx).expect("editor values");
        assert_eq!(values.name, "Edit Me");
        assert_eq!(values.host, "old.example");
        assert_eq!(values.port, "2222");
        assert_eq!(values.username, "alice");
        assert!(values.key_auth);
        assert_eq!(values.key_path, "~/.ssh/id_ed25519");
        assert_eq!(values.group_path, "Prod / Web");
        assert!(!values.group_changed);
    });
    // List navigation keys belong to the text field while the editor is open.
    harness.visual.simulate_keystrokes("up");
    harness.visual.simulate_keystrokes("home");
    harness.visual.simulate_keystrokes("end");
    harness.visual.update(|_, cx| {
        assert_eq!(picker(&root, cx).selected_profile_id(), Some("edit-me"));
    });
    harness.visual.simulate_input(" Updated");
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        let values = picker.editor_values_for_test(cx).expect("editor values");
        assert_eq!(values.name, "Edit Me Updated");
        assert_eq!(values.host, "old.example");
        assert_eq!(values.port, "2222");
        assert_eq!(values.username, "alice");
        assert!(values.key_auth);
        assert_eq!(values.key_path, "~/.ssh/id_ed25519");
        assert_eq!(values.group_path, "Prod / Web");
        assert!(!values.group_changed);
        let draft = build_draft(Some("edit-me".to_string()), values);
        assert_eq!(draft.id.as_deref(), Some("edit-me"));
        assert_eq!(draft.host, "old.example");
        assert_eq!(draft.port, "2222");
        assert_eq!(draft.username, "alice");
        assert!(matches!(draft.auth, AuthKind::Key));
        assert_eq!(draft.key_path, "~/.ssh/id_ed25519");
        assert_eq!(draft.group_path, "Prod / Web");
        assert!(!draft.group_changed);
    });
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(
                Ok(vec![
                    summary("alpha", "Alpha", 22),
                    summary("edit-me", "Edit Me Updated", 2222),
                ]),
                cx,
            );
        });
    });
    click(harness.visual, EDITOR_SAVE_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(!picker.editor_is_open());
        assert_eq!(picker.selected_profile_id(), Some("edit-me"));
        assert_eq!(picker.profiles()[1].name, "Edit Me Updated");
    });
}

#[gpui_kit::test]
fn a_save_without_a_host_or_user_stays_in_the_editor(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(Vec::new()), cx);
            picker.set_editor_save_for_test(Ok("never-saved".to_string()));
        });
    });
    open_picker(&mut harness);
    click(harness.visual, EMPTY_NEW_BUTTON_ID);
    click(harness.visual, EDITOR_SAVE_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(picker.editor_is_open(), "validation must not save");
        assert!(picker.editor_error_for_test().is_some());
        assert!(!picker.editor_name_focused_for_test(window, cx));
    });
    focus_field(&mut harness, EditorField::Host);
    harness.visual.simulate_input("example.com");
    click(harness.visual, EDITOR_SAVE_ID);
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert!(picker.editor_is_open(), "the user name is still required");
        assert!(picker.editor_error_for_test().is_some());
    });
    focus_field(&mut harness, EditorField::Username);
    harness.visual.simulate_input("deploy");
    focus_field(&mut harness, EditorField::Port);
    harness.visual.simulate_input("abc");
    click(harness.visual, EDITOR_SAVE_ID);
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert!(picker.editor_is_open(), "an invalid port must not save");
        assert!(picker.editor_error_for_test().is_some());
    });
}

#[test]
fn draft_building_defaults_the_port_and_keeps_hidden_fields() {
    let values = EditorValues {
        name: "Box".to_string(),
        host: "example.com".to_string(),
        port: String::new(),
        username: "root".to_string(),
        key_auth: false,
        key_path: "~/.ssh/keep".to_string(),
        group_path: "Prod / Web".to_string(),
        group_changed: true,
    };
    assert_eq!(invalid_field(&values), None);
    let draft = build_draft(None, values);
    assert_eq!(draft.port, "22");
    assert_eq!(draft.key_path, "~/.ssh/keep");
    assert_eq!(draft.group_path, "Prod / Web");
    assert!(draft.group_changed);
    assert!(matches!(draft.auth, AuthKind::Password));
    let invalid_port = EditorValues {
        name: "Box".to_string(),
        host: "example.com".to_string(),
        port: "0".to_string(),
        username: "root".to_string(),
        key_auth: false,
        key_path: String::new(),
        group_path: String::new(),
        group_changed: false,
    };
    assert_eq!(invalid_field(&invalid_port), Some(EditorField::Port));
    let missing_key = EditorValues {
        name: "Box".to_string(),
        host: "example.com".to_string(),
        port: "22".to_string(),
        username: "root".to_string(),
        key_auth: true,
        key_path: String::new(),
        group_path: String::new(),
        group_changed: false,
    };
    assert_eq!(invalid_field(&missing_key), Some(EditorField::KeyPath));
}

#[test]
fn an_untouched_group_is_told_apart_from_one_the_user_changed() {
    assert!(!group_path_changed("", ""));
    assert!(!group_path_changed(" Prod /Web ", "Prod / Web"));
    assert!(group_path_changed("", "Prod / Web"));
    assert!(group_path_changed("Prod / API", "Prod / Web"));
}
