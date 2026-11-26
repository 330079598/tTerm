//! Regressions for Tab reachability and nested long-list scrolling.
//!
//! These use the real picker window and synthesized keys, but only in-memory
//! profile summaries: no stored profile, key or network connection is read.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{App, Entity, MouseButton, TestAppContext, VisualTestContext, px};
use tterm_core::config::AppConfig;
use tterm_core::profiles::launch::SavedSshProfileSummary;

use super::super::{LIST_ID, OPEN_BUTTON_ID, ROW_ID_PREFIX, SavedConnectionsUi};
use super::{GROUP_ID_PREFIX, ProfileSort, SORT_GROUP_ID, SORT_RECENT_ID};
use crate::terminal_demo::command::CommandSpec;
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};
use crate::terminal_demo::window::Prototype;
use crate::terminal_demo::window::split_session_tests::make_workspace_with;

fn summary(
    id: &str,
    name: &str,
    host: &str,
    username: &str,
    group: Option<&str>,
) -> SavedSshProfileSummary {
    SavedSshProfileSummary {
        id: id.to_string(),
        name: name.to_string(),
        username: Some(username.to_string()),
        host: Some(host.to_string()),
        port: Some(22),
        group_path: group.map(str::to_string),
    }
}

fn grouped_profiles() -> Vec<SavedSshProfileSummary> {
    vec![
        summary("web", "Web", "web.example", "deploy", Some("Prod / Web")),
        summary("api", "API", "api.example", "alice", Some("Prod / API")),
        summary("loose", "Loose", "loose.example", "me", None),
    ]
}

fn long_library() -> Vec<SavedSshProfileSummary> {
    (0..48)
        .map(|index| {
            let group = match index % 3 {
                0 => "Library / Alpha",
                1 => "Library / Beta / North",
                _ => "Library / Beta / South",
            };
            summary(
                &format!("row-{index:02}"),
                &format!("Row {index:02}"),
                &format!("row{index}.example"),
                "tester",
                Some(group),
            )
        })
        .collect()
}

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

fn press(harness: &mut Harness<'_>, keys: &str) {
    harness.visual.simulate_keystrokes(keys);
    harness.visual.update(|window, cx| window.render_frame(cx));
}

#[gpui_kit::test]
fn tab_reaches_sort_choices_and_group_headings_without_opening(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(grouped_profiles()), cx);
        });
    });
    open_picker(&mut harness);
    harness.visual.update(|window, cx| {
        assert!(picker(&root, cx).search_focused_for_test(window, cx));
    });

    press(&mut harness, "tab");
    harness.visual.update(|window, cx| {
        assert!(
            picker(&root, cx)
                .navigation
                .sort_focus(SORT_GROUP_ID, cx)
                .is_focused(window),
            "Tab should reach By group"
        );
    });

    press(&mut harness, "tab");
    harness.visual.update(|window, cx| {
        assert!(
            picker(&root, cx)
                .navigation
                .sort_focus(SORT_RECENT_ID, cx)
                .is_focused(window),
            "Tab should reach Recently used"
        );
    });

    press(&mut harness, "enter");
    harness.visual.update(|_, cx| {
        assert_eq!(picker(&root, cx).sort_for_test(), ProfileSort::Recent);
        assert!(!picker(&root, cx).is_opening());
    });

    press(&mut harness, "shift-tab");
    press(&mut harness, "space");
    harness.visual.update(|_, cx| {
        assert_eq!(picker(&root, cx).sort_for_test(), ProfileSort::Group);
        assert!(!picker(&root, cx).is_opening());
    });

    press(&mut harness, "tab");
    press(&mut harness, "tab");
    harness.visual.update(|window, cx| {
        assert!(
            picker(&root, cx).group_header_focused(window, cx),
            "Tab should reach the first group heading"
        );
    });

    press(&mut harness, "enter");
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        let visible = picker.visible_ids_for_test();
        assert!(!visible.iter().any(|id| id == "web" || id == "api"));
        assert_eq!(picker.selected_profile_id(), Some("loose"));
        assert!(!picker.is_opening(), "Enter on a heading must not connect");
    });

    press(&mut harness, "space");
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        let visible = picker.visible_ids_for_test();
        assert!(visible.iter().any(|id| id == "web"));
        assert!(visible.iter().any(|id| id == "api"));
        assert!(!picker.is_opening(), "Space on a heading must not connect");
        assert!(picker.is_open());
    });
}

#[gpui_kit::test]
fn end_home_and_wrap_keep_a_nested_long_list_row_in_view(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(long_library()), cx);
        });
    });
    open_picker(&mut harness);
    harness.visual.update(|window, cx| window.render_frame(cx));

    let visible = harness
        .visual
        .update(|_, cx| picker(&root, cx).visible_ids_for_test());
    assert_eq!(visible.len(), 48);
    let first_id = visible.first().cloned().expect("the first row");
    let last_id = visible.last().cloned().expect("the last row");
    let last_index = visible.len() - 1;
    let nested: &'static str =
        Box::leak(format!("{GROUP_ID_PREFIX}Library / Beta / North").into_boxed_str());
    assert!(
        harness.visual.debug_bounds(nested).is_some(),
        "the long list should keep its nested group headings"
    );

    press(&mut harness, "end");
    harness.visual.update(|window, cx| window.render_frame(cx));
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert_eq!(picker.selected_profile_id(), Some(last_id.as_str()));
        assert!(
            picker.scroll_handle.offset().y < px(0.0),
            "End must scroll the nested list"
        );
    });
    assert_visible(harness.visual, LIST_ID, last_index, Edge::Bottom);

    press(&mut harness, "home");
    harness.visual.update(|window, cx| window.render_frame(cx));
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert_eq!(picker.selected_profile_id(), Some(first_id.as_str()));
        assert!(picker.scroll_handle.offset().y <= px(0.0));
    });
    assert_visible(harness.visual, LIST_ID, 0, Edge::Top);

    press(&mut harness, "up");
    harness.visual.update(|window, cx| window.render_frame(cx));
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert_eq!(picker.selected_profile_id(), Some(last_id.as_str()));
        assert!(picker.scroll_handle.offset().y < px(0.0));
    });
    assert_visible(harness.visual, LIST_ID, last_index, Edge::Bottom);
}

enum Edge {
    Top,
    Bottom,
}

/// Asserts a visible row's real painted bounds stay inside the list viewport
/// and that it reached the nearest edge rather than jumping to the far one.
fn assert_visible(
    visual: &mut VisualTestContext,
    list_selector: &'static str,
    row_index: usize,
    edge: Edge,
) {
    let list = visual
        .debug_bounds(list_selector)
        .unwrap_or_else(|| panic!("{list_selector} should be laid out"));
    let selector: &'static str = Box::leak(format!("{ROW_ID_PREFIX}{row_index}").into_boxed_str());
    let row = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} should be laid out"));
    assert!(
        row.top() >= list.top() - px(0.5),
        "row top should be inside the list viewport"
    );
    assert!(
        row.bottom() <= list.bottom() + px(0.5),
        "row bottom should be inside the list viewport"
    );
    match edge {
        Edge::Top => assert!(
            row.top() <= list.top() + px(16.0),
            "the first row should use the nearest top edge"
        ),
        Edge::Bottom => assert!(
            row.top() > list.top() + px(16.0),
            "the last row should use the nearest bottom edge"
        ),
    }
}
