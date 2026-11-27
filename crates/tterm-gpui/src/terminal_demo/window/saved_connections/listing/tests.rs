//! Regressions for saved-connection grouping, search and visible selection.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{App, Entity, MouseButton, TestAppContext, VisualTestContext};
use tterm_core::config::AppConfig;
use tterm_core::connections::profile_recents::RecentProfile;
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::profiles::types::ProfileGroup;

use super::super::delete_selected::{DELETE_BUTTON_ID, DELETE_CANCEL_ID, DELETE_CONFIRM_ID};
use super::super::move_to_group::{MOVE_BUTTON_ID, MOVE_GROUP_PREFIX, ROW_TOGGLE_PREFIX};
use super::super::{
    EDIT_BUTTON_ID, OPEN_BUTTON_ID, ROW_EDIT_PREFIX, ROW_ID_PREFIX, SavedConnectionsUi,
};
use super::{
    GROUP_ID_PREFIX, ListingOptions, ProfileListing, ProfileSort, SEARCH_EMPTY_ID, SORT_GROUP_ID,
    SORT_RECENT_ID, UNGROUPED_KEY,
};
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

fn listing(profiles: &[SavedSshProfileSummary]) -> ProfileListing {
    let mut listing = ProfileListing::default();
    listing.replace_profiles(profiles);
    listing
}

fn ids(listing: &ProfileListing) -> Vec<&str> {
    listing.visible_ids().iter().map(String::as_str).collect()
}

#[test]
fn nests_group_paths_and_counts_every_profile_inside() {
    let listing = listing(&[
        summary("web", "Web", "web.example", "deploy", Some("Prod / Web")),
        summary("api", "API", "api.example", "deploy", Some("Prod / API")),
        summary("dev", "Dev", "dev.example", "me", Some("Dev")),
        summary("loose", "Loose", "loose.example", "me", None),
    ]);

    let roots: Vec<&str> = listing
        .roots()
        .iter()
        .map(|node| node.group.name.as_str())
        .collect();
    assert_eq!(roots, ["Dev", "Prod"]);

    let prod = &listing.roots()[1];
    assert_eq!(prod.total, 2);
    assert_eq!(prod.path, "Prod");
    let children: Vec<&str> = prod
        .children
        .iter()
        .map(|node| node.group.name.as_str())
        .collect();
    assert_eq!(children, ["API", "Web"]);
    assert_eq!(prod.children[0].path, "Prod / API");
    assert_eq!(ids(&listing), ["dev", "api", "web", "loose"]);
    assert_eq!(
        listing
            .ungrouped()
            .iter()
            .map(|profile| profile.id.as_str())
            .collect::<Vec<_>>(),
        ["loose"]
    );
}

#[test]
fn searching_matches_name_host_username_and_the_full_group_path() {
    let mut listing = listing(&[
        summary("web", "Web", "web.example", "deploy", Some("Prod / Web")),
        summary("api", "API", "api.example", "alice", Some("Prod / API")),
        summary("dev", "Dev", "dev.example", "me", None),
    ]);

    let cases = [
        ("WEB", vec!["web"]),
        ("api.example", vec!["api"]),
        ("ALICE", vec!["api"]),
        ("prod / web", vec!["web"]),
        ("  prod   web  ", vec!["web"]),
        ("nothing", Vec::new()),
    ];
    for (query, expected) in cases {
        listing.set_query(query.to_string());
        assert_eq!(ids(&listing), expected, "query {query:?}");
    }
}

#[test]
fn an_exact_name_outranks_a_prefix_and_a_username_match() {
    let mut listing = listing(&[
        summary("user", "gateway", "gateway.example", "web", None),
        summary("prefix", "web-02", "web.example", "me", None),
        summary("exact", "web", "exact.example", "me", None),
    ]);
    listing.set_query("web".to_string());
    assert_eq!(ids(&listing), ["exact", "prefix", "user"]);
}

#[test]
fn collapsing_groups_and_ungrouped_hides_their_rows() {
    let mut listing = listing(&[
        summary("web", "Web", "web.example", "deploy", Some("Prod / Web")),
        summary("api", "API", "api.example", "deploy", Some("Prod / API")),
        summary("dev", "Dev", "dev.example", "me", Some("Dev")),
        summary("loose", "Loose", "loose.example", "me", None),
    ]);

    assert!(listing.toggle_group("Prod"));
    assert_eq!(ids(&listing), ["dev", "loose"]);
    assert_eq!(listing.active_id(Some("web")).as_deref(), Some("dev"));

    assert!(listing.toggle_group(UNGROUPED_KEY));
    assert_eq!(ids(&listing), ["dev"]);
    assert!(listing.toggle_group("Prod"));
    assert_eq!(ids(&listing), ["dev", "api", "web"]);
    assert_eq!(listing.active_id(Some("loose")).as_deref(), Some("dev"));
}

#[test]
fn no_search_results_leave_no_visible_target() {
    let mut listing = listing(&[summary(
        "web",
        "Web",
        "web.example",
        "deploy",
        Some("Prod / Web"),
    )]);
    listing.set_query("zzz".to_string());
    assert!(listing.visible_is_empty());
    assert_eq!(listing.active_id(Some("web")), None);
}

#[test]
fn recent_order_puts_the_last_used_first_and_wins_over_groups() {
    let mut listing = listing(&[
        summary("web", "Web", "web.example", "deploy", Some("Prod / Web")),
        summary("api", "API", "api.example", "alice", Some("Prod / API")),
    ]);
    listing.set_recents(&[
        RecentProfile {
            profile_id: "api".to_string(),
            last_used_at: 2,
        },
        RecentProfile {
            profile_id: "web".to_string(),
            last_used_at: 1,
        },
    ]);
    listing.set_sort(ProfileSort::Recent);
    assert_eq!(ids(&listing), ["api", "web"]);
    assert!(listing.is_flat());
    assert_eq!(listing.active_id(Some("web")).as_deref(), Some("web"));
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

fn grouped_profiles() -> Vec<SavedSshProfileSummary> {
    vec![
        summary("web", "Web", "web.example", "deploy", Some("Prod / Web")),
        summary("api", "API", "api.example", "alice", Some("Prod / API")),
        summary("loose", "Loose", "loose.example", "me", None),
    ]
}

#[gpui_kit::test]
fn search_and_collapse_keep_keyboard_on_a_visible_row(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(grouped_profiles()), cx);
        });
    });
    open_picker(&mut harness);

    harness.visual.update(|window, cx| {
        let picker = picker(&root, cx);
        assert!(picker.search_focused_for_test(window, cx));
        assert_eq!(picker.selected_profile_id(), Some("api"));
    });

    harness.visual.simulate_input("alice");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(picker.visible_ids_for_test(), vec!["api".to_string()]);
        assert_eq!(picker.selected_profile_id(), Some("api"));
        assert_eq!(picker.search_value_for_test(cx), "alice");
    });
    click(harness.visual, &format!("{ROW_ID_PREFIX}0"));
    harness.visual.update(|_, cx| {
        assert_eq!(picker(&root, cx).selected_profile_id(), Some("api"));
        assert_eq!(
            picker(&root, cx).visible_ids_for_test(),
            vec!["api".to_string()]
        );
    });

    harness.visual.simulate_input("zzz");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(picker.visible_ids_for_test().is_empty());
        assert_eq!(picker.selected_profile_id(), None);
        assert!(picker.active_profile_id().is_none());
    });
    assert!(harness.visual.debug_bounds(SEARCH_EMPTY_ID).is_some());
    harness.visual.simulate_keystrokes("enter");
    harness.visual.update(|_, cx| {
        assert!(picker(&root, cx).is_open());
        assert!(!picker(&root, cx).is_opening());
    });

    for _ in 0.."alicezzz".len() {
        harness.visual.simulate_keystrokes("backspace");
    }
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(picker.search_value_for_test(cx), "");
        assert!(!picker.visible_ids_for_test().is_empty());
        assert_eq!(
            picker.selected_profile_id(),
            picker.visible_ids_for_test().first().map(String::as_str)
        );
    });

    // A hidden selected row must not survive collapsing its group.
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.select_profile("web", cx);
        });
    });
    click(harness.visual, &format!("{GROUP_ID_PREFIX}Prod"));
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        let visible = picker.visible_ids_for_test();
        assert!(!visible.iter().any(|id| id == "web"));
        assert_eq!(picker.selected_profile_id(), Some("loose"));
    });
    let edit_selector: &'static str = Box::leak(format!("{ROW_EDIT_PREFIX}0").into_boxed_str());
    assert!(
        harness.visual.debug_bounds(edit_selector).is_some(),
        "the visible fallback row keeps its edit action"
    );
}

#[gpui_kit::test]
fn a_group_heading_toggles_from_the_keyboard(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(grouped_profiles()), cx);
        });
    });
    open_picker(&mut harness);
    harness.visual.update(|window, cx| {
        let picker = root.read(cx).ui.saved_connections.clone();
        picker.update(cx, |picker, cx| {
            picker.focus_group_for_test("Prod", window, cx);
        });
    });
    harness.visual.simulate_keystrokes("enter");
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        let visible = picker.visible_ids_for_test();
        assert!(!visible.iter().any(|id| id == "web" || id == "api"));
        assert_eq!(picker.selected_profile_id(), Some("loose"));
    });
    harness.visual.update(|window, cx| {
        let picker = root.read(cx).ui.saved_connections.clone();
        picker.update(cx, |picker, cx| {
            picker.focus_group_for_test("Prod", window, cx);
        });
    });
    harness.visual.simulate_keystrokes("space");
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert!(picker.visible_ids_for_test().iter().any(|id| id == "web"));
    });
}

#[gpui_kit::test]
fn the_sort_toggle_moves_between_groups_and_recently_used(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(grouped_profiles()), cx);
            picker.set_listing_options_for_test(
                ListingOptions {
                    sort: ProfileSort::Group,
                    recents: vec![
                        RecentProfile {
                            profile_id: "loose".to_string(),
                            last_used_at: 9,
                        },
                        RecentProfile {
                            profile_id: "web".to_string(),
                            last_used_at: 3,
                        },
                    ],
                },
                cx,
            );
        });
    });
    open_picker(&mut harness);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(picker(&root, cx).sort_for_test(), ProfileSort::Group);
        assert_eq!(
            picker(&root, cx).visible_ids_for_test(),
            vec!["api".to_string(), "web".to_string(), "loose".to_string()]
        );
    });

    click(harness.visual, SORT_RECENT_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(picker.sort_for_test(), ProfileSort::Recent);
        assert_eq!(
            picker.visible_ids_for_test(),
            vec!["loose".to_string(), "web".to_string(), "api".to_string()]
        );
        assert_eq!(picker.selected_profile_id(), Some("loose"));
    });

    click(harness.visual, SORT_GROUP_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(picker.sort_for_test(), ProfileSort::Group);
        assert_eq!(
            picker.visible_ids_for_test(),
            vec!["api".to_string(), "web".to_string(), "loose".to_string()]
        );
        assert_eq!(picker.selected_profile_id(), Some("api"));
    });
}

#[gpui_kit::test]
fn edit_and_open_are_disabled_when_the_search_has_no_results(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(grouped_profiles()), cx);
        });
    });
    open_picker(&mut harness);
    harness.visual.simulate_input("no such connection");

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(picker.active_profile_id().is_none());
        assert_eq!(picker.selected_profile_id(), None);
        assert!(picker.error().is_none());
        assert!(picker.is_open());
    });
    harness.visual.simulate_keystrokes("enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(picker.is_open(), "Enter must not open a hidden profile");
        assert!(!picker.is_opening());
        assert!(picker.active_profile_id().is_none());
    });
    assert!(harness.visual.debug_bounds(EDIT_BUTTON_ID).is_some());
    let edit_selector: &'static str = Box::leak(format!("{ROW_EDIT_PREFIX}0").into_boxed_str());
    assert!(harness.visual.debug_bounds(edit_selector).is_none());
}

#[gpui_kit::test]
fn checked_rows_move_to_the_chosen_existing_group(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(grouped_profiles()), cx);
            picker.set_move_groups_for_test(
                Ok(vec![ProfileGroup {
                    id: "prod".to_string(),
                    name: "Prod".to_string(),
                    parent_id: None,
                }]),
                cx,
            );
            picker.set_move_result_for_test(Ok(()));
        });
    });
    open_picker(&mut harness);

    click(harness.visual, &format!("{ROW_TOGGLE_PREFIX}0"));
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(picker.checked_ids_for_test(), ["api"]);
        assert_eq!(
            picker.visible_ids_for_test(),
            vec!["api".to_string(), "web".to_string(), "loose".to_string()]
        );
    });
    click(harness.visual, &format!("{ROW_TOGGLE_PREFIX}1"));
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(picker.checked_ids_for_test(), ["api", "web"]);
    });

    click(harness.visual, MOVE_BUTTON_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert!(picker.move_picker_is_open());
        assert!(picker.last_move_request_for_test().is_none());
    });

    click(harness.visual, &format!("{MOVE_GROUP_PREFIX}prod"));
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let picker = picker(&root, cx);
        assert_eq!(
            picker.last_move_request_for_test(),
            Some((
                vec!["api".to_string(), "web".to_string()],
                Some("prod".to_string())
            ))
        );
        assert!(picker.checked_ids_for_test().is_empty());
        assert!(!picker.move_picker_is_open());
        assert!(picker.notice().is_some());
        assert!(picker.is_open());
    });
}

#[gpui_kit::test]
fn checked_rows_delete_only_after_the_confirmation(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(grouped_profiles()), cx);
            picker.set_delete_result_for_test(Ok(()));
        });
    });
    open_picker(&mut harness);

    click(harness.visual, &format!("{ROW_TOGGLE_PREFIX}0"));
    click(harness.visual, &format!("{ROW_TOGGLE_PREFIX}1"));
    harness.visual.update(|_, cx| {
        assert_eq!(picker(&root, cx).checked_ids_for_test(), ["api", "web"]);
        assert!(picker(&root, cx).last_delete_request_for_test().is_none());
    });

    click(harness.visual, DELETE_BUTTON_ID);
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert!(picker.delete_confirm_is_open());
        assert!(
            picker.last_delete_request_for_test().is_none(),
            "opening the confirmation must not delete anything"
        );
        assert_eq!(picker.checked_ids_for_test(), ["api", "web"]);
    });

    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, _cx| {
            picker.set_next_load_for_test(Ok(vec![summary(
                "loose",
                "Loose",
                "loose.example",
                "me",
                None,
            )]));
        });
    });
    click(harness.visual, DELETE_CONFIRM_ID);
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert_eq!(
            picker.last_delete_request_for_test(),
            Some(vec!["api".to_string(), "web".to_string()])
        );
        assert!(picker.checked_ids_for_test().is_empty());
        assert!(!picker.delete_confirm_is_open());
        assert!(picker.notice().is_some());
        assert!(picker.is_open());
        assert_eq!(picker.visible_ids_for_test(), vec!["loose".to_string()]);
    });
}

#[gpui_kit::test]
fn cancelling_the_delete_confirmation_keeps_the_checked_rows(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(grouped_profiles()), cx);
            picker.set_delete_result_for_test(Ok(()));
        });
    });
    open_picker(&mut harness);

    click(harness.visual, &format!("{ROW_TOGGLE_PREFIX}0"));
    click(harness.visual, DELETE_BUTTON_ID);
    click(harness.visual, DELETE_CANCEL_ID);

    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert!(!picker.delete_confirm_is_open());
        assert_eq!(picker.checked_ids_for_test(), ["api"]);
        assert!(picker.last_delete_request_for_test().is_none());
    });
}
