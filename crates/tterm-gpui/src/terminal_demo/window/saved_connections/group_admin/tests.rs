//! Regressions for the group panel: create, rename, delete and cancel.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{App, Entity, MouseButton, TestAppContext, VisualTestContext};
use tterm_core::config::AppConfig;
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::profiles::types::ProfileGroup;

use super::super::{OPEN_BUTTON_ID, SavedConnectionsUi};
use super::{
    GROUP_ADMIN_BUTTON_ID, GROUP_ADMIN_CREATE_ID, GROUP_ADMIN_DELETE_ID, GROUP_ADMIN_PANEL_ID,
    GROUP_ADMIN_RENAME_ID, GROUP_ADMIN_ROW_PREFIX, GroupChange,
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

fn click(visual: &mut VisualTestContext, selector: &str) {
    let selector: &'static str = Box::leak(selector.to_string().into_boxed_str());
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

fn group(id: &str, name: &str) -> ProfileGroup {
    ProfileGroup {
        id: id.to_string(),
        name: name.to_string(),
        parent_id: None,
    }
}

fn summary(id: &str, name: &str, group_path: Option<&str>) -> SavedSshProfileSummary {
    SavedSshProfileSummary {
        id: id.to_string(),
        name: name.to_string(),
        username: Some("me".to_string()),
        host: Some("host.example".to_string()),
        port: Some(22),
        group_path: group_path.map(str::to_string),
    }
}

fn open_group_panel(
    harness: &mut Harness<'_>,
    groups: Vec<ProfileGroup>,
    profiles: Vec<SavedSshProfileSummary>,
    change: Vec<ProfileGroup>,
) {
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(profiles), cx);
            picker.set_group_admin_groups_for_test(Ok(groups), cx);
            picker.set_group_change_for_test(Ok(change));
        });
    });
    open_picker(harness);
    click(harness.visual, GROUP_ADMIN_BUTTON_ID);
    harness.visual.update(|window, cx| window.render_frame(cx));
    assert!(
        harness.visual.debug_bounds(GROUP_ADMIN_PANEL_ID).is_some(),
        "the group panel should be open"
    );
}

#[gpui_kit::test]
fn creating_a_group_sends_the_trimmed_name_and_keeps_the_panel_open(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_group_panel(
        &mut harness,
        Vec::new(),
        Vec::new(),
        vec![group("prod", "Prod")],
    );

    harness.visual.simulate_input("  Prod  ");
    click(harness.visual, GROUP_ADMIN_CREATE_ID);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(
            picker.last_group_change_for_test(),
            Some(GroupChange::Create {
                name: "Prod".to_string()
            })
        );
        assert_eq!(picker.group_admin_names_for_test(), ["Prod"]);
        assert_eq!(picker.group_admin_name_value_for_test(cx), "");
        assert_eq!(picker.group_admin_selected_for_test(), None);
        assert!(picker.group_admin_is_open());
    });
    assert!(harness.visual.debug_bounds(GROUP_ADMIN_PANEL_ID).is_some());
}

#[gpui_kit::test]
fn selecting_a_group_fills_the_name_field_and_rename_uses_it(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_group_panel(
        &mut harness,
        vec![group("prod", "Prod"), group("dev", "Dev")],
        Vec::new(),
        vec![group("prod", "Production"), group("dev", "Dev")],
    );

    click(harness.visual, &format!("{GROUP_ADMIN_ROW_PREFIX}prod"));
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(picker.group_admin_selected_for_test(), Some("prod"));
        assert_eq!(picker.group_admin_name_value_for_test(cx), "Prod");
    });
    harness.visual.update(|window, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_group_admin_name_for_test("Production", window, cx);
        });
    });

    click(harness.visual, GROUP_ADMIN_RENAME_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(
            picker.last_group_change_for_test(),
            Some(GroupChange::Rename {
                id: "prod".to_string(),
                name: "Production".to_string()
            })
        );
        assert_eq!(picker.group_admin_names_for_test(), ["Production", "Dev"]);
        assert_eq!(picker.group_admin_selected_for_test(), Some("prod"));
    });
}

#[gpui_kit::test]
fn deleting_a_group_needs_a_second_click(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_group_panel(
        &mut harness,
        vec![group("prod", "Prod"), group("dev", "Dev")],
        vec![summary("web", "Web", Some("Prod"))],
        vec![group("prod", "Prod")],
    );

    click(harness.visual, &format!("{GROUP_ADMIN_ROW_PREFIX}dev"));
    click(harness.visual, GROUP_ADMIN_DELETE_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(picker.group_admin_confirming_delete_for_test());
        assert_eq!(picker.last_group_change_for_test(), None);
    });

    click(harness.visual, GROUP_ADMIN_DELETE_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(
            picker.last_group_change_for_test(),
            Some(GroupChange::Delete {
                id: "dev".to_string()
            })
        );
        assert_eq!(picker.group_admin_names_for_test(), ["Prod"]);
        assert_eq!(picker.group_admin_selected_for_test(), None);
        assert!(!picker.group_admin_confirming_delete_for_test());
    });
}

#[gpui_kit::test]
fn an_empty_name_reports_the_required_error_without_a_request(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_group_panel(&mut harness, Vec::new(), Vec::new(), Vec::new());

    click(harness.visual, GROUP_ADMIN_CREATE_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(picker.group_admin_error_for_test().is_some());
        assert_eq!(picker.last_group_change_for_test(), None);
        assert!(picker.group_admin_is_open());
    });
}

#[gpui_kit::test]
fn escape_closes_the_group_panel_and_keeps_the_picker_open(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_group_panel(
        &mut harness,
        vec![group("prod", "Prod")],
        Vec::new(),
        Vec::new(),
    );

    harness.visual.simulate_keystrokes("escape");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(!picker.group_admin_is_open());
        assert!(picker.is_open());
    });
    assert!(harness.visual.debug_bounds(GROUP_ADMIN_PANEL_ID).is_none());
}
