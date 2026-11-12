use crate::workspace::tab::Tab;
use crate::workspace::tab_groups::{
    PaneNode, SplitOrientation, TabGroup, pane_leaf, remove_tabs_from_groups,
};
use crate::workspace::tabs_state::reopen::history::{
    CLOSED_TABS_LIMIT, closed_entries, closed_tab_snapshot, pane_anchor, push_closed_entries,
};
use crate::workspace::tabs_state::reopen::{ClosedLayout, PaneAnchor, place_reopened_tabs};
use serde_json::{Value, json};

fn tab(id: &str, overrides: Value) -> Tab {
    let mut value = json!({"id": id, "title": id, "type": "terminal", "isActive": false, "sessionNonce": 3, "pid": 42, "connection": {"type": "terminal", "cwd": format!("/work/{id}")}});
    value
        .as_object_mut()
        .unwrap()
        .extend(overrides.as_object().unwrap().clone());
    serde_json::from_value(value).unwrap()
}

fn split(orientation: SplitOrientation, children: Vec<PaneNode>, sizes: Vec<f64>) -> PaneNode {
    PaneNode::Split {
        orientation,
        children,
        sizes,
    }
}

fn split_group() -> TabGroup {
    TabGroup {
        id: "g1".into(),
        root: split(
            SplitOrientation::Horizontal,
            vec![
                pane_leaf("web"),
                split(
                    SplitOrientation::Vertical,
                    vec![pane_leaf("api"), pane_leaf("logs")],
                    vec![0.5, 0.5],
                ),
            ],
            vec![0.6, 0.4],
        ),
        active_tab_id: "api".into(),
        zoomed_tab_id: None,
    }
}

fn single() -> TabGroup {
    TabGroup {
        id: "g2".into(),
        root: pane_leaf("solo"),
        active_tab_id: "solo".into(),
        zoomed_tab_id: None,
    }
}

fn all_tabs() -> Vec<Tab> {
    vec![
        tab("web", json!({})),
        tab("api", json!({})),
        tab("logs", json!({"type": "settings"})),
        tab("solo", json!({})),
    ]
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

#[test]
fn keeps_a_local_tabs_directory_but_not_its_session() {
    let snapshot = closed_tab_snapshot(&tab("a", json!({"autoTitle": true}))).unwrap();
    assert_eq!(snapshot.field("title"), Some(&json!("a")));
    assert_eq!(snapshot.field("autoTitle"), Some(&json!(true)));
    assert_eq!(snapshot.field("connection").unwrap()["cwd"], "/work/a");
    for key in ["id", "pid", "sessionNonce"] {
        assert_eq!(snapshot.field(key), None);
    }
}

#[test]
fn forgets_the_passwords_typed_for_a_connection() {
    let snapshot = closed_tab_snapshot(&tab("b", json!({"type": "ssh", "quickConnect": true, "connection": {"type": "ssh", "host": "web", "password": "secret", "privateKeyPassphrase": "phrase", "jumpHosts": [{"host": "bastion", "port": 22, "username": "me", "authMethod": "password", "password": "x"}]}}))).unwrap();
    assert_eq!(snapshot.field("quickConnect"), Some(&json!(false)));
    let connection = snapshot.field("connection").unwrap();
    assert_eq!(connection["host"], "web");
    assert_eq!(connection.get("password"), None);
    assert_eq!(connection.get("privateKeyPassphrase"), None);
    assert_eq!(connection["jumpHosts"][0]["host"], "bastion");
    assert_eq!(connection["jumpHosts"][0].get("password"), None);
}

#[test]
fn keeps_no_pages_or_remote_files() {
    assert_eq!(
        closed_tab_snapshot(&tab("s", json!({"type": "settings"}))),
        None
    );
    assert_eq!(
        closed_tab_snapshot(&tab("f", json!({"type": "remote-file-editor"}))),
        None
    );
}

#[test]
fn keeps_a_whole_tab_of_the_bar_with_its_split_leaving_out_its_pages() {
    let entries = closed_entries(
        &all_tabs(),
        &[single(), split_group()],
        &ids(&["web", "api", "logs"]),
    );
    assert_eq!(
        entries[0]
            .tabs
            .iter()
            .map(|tab| tab.field("title").unwrap().as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["web", "api"]
    );
    assert_eq!(
        serde_json::to_value(&entries[0].layout).unwrap(),
        json!({"kind": "group", "shape": {"type": "split", "orientation": "horizontal", "children": [{"type": "pane"}, {"type": "pane"}], "sizes": [0.6, 0.4]}, "activeIndex": 1, "index": 1})
    );
}

#[test]
fn keeps_a_pane_closed_out_of_a_tab_that_stays_open_with_that_tabs_split() {
    let group = split_group();
    let entries = closed_entries(&all_tabs(), std::slice::from_ref(&group), &ids(&["api"]));
    assert_eq!(
        entries[0]
            .tabs
            .iter()
            .map(|tab| tab.field("title").unwrap().as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["api"]
    );
    assert_eq!(
        entries[0].layout,
        ClosedLayout::Pane {
            closed_id: "api".into(),
            previous_root: group.root,
            anchor: Some(PaneAnchor {
                tab_id: "logs".into(),
                direction: crate::workspace::tab_groups::SplitDirection::Above
            })
        }
    );
}

#[test]
fn makes_one_closing_for_each_tab_of_the_bar_closed_in_the_bars_order() {
    let entries = closed_entries(
        &all_tabs(),
        &[single(), split_group()],
        &ids(&["solo", "web", "api", "logs"]),
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| serde_json::to_value(&entry.layout).unwrap()["kind"].clone())
            .collect::<Vec<_>>(),
        vec![json!("group"), json!("group")]
    );
    assert_eq!(entries[0].tabs[0].field("title"), Some(&json!("solo")));
}

#[test]
fn goes_back_after_the_pane_before_it_else_before_the_one_after_it() {
    use crate::workspace::tab_groups::SplitDirection::{Below, Left};
    assert_eq!(
        pane_anchor(&split_group().root, "logs"),
        Some(PaneAnchor {
            tab_id: "api".into(),
            direction: Below
        })
    );
    assert_eq!(
        pane_anchor(&split_group().root, "web"),
        Some(PaneAnchor {
            tab_id: "api".into(),
            direction: Left
        })
    );
    assert_eq!(pane_anchor(&pane_leaf("solo"), "solo"), None);
}

#[test]
fn puts_a_whole_tab_of_the_bar_back_in_its_place_with_its_split() {
    let entries = closed_entries(
        &all_tabs(),
        &[single(), split_group()],
        &ids(&["web", "api", "logs"]),
    );
    let groups = place_reopened_tabs(
        &[single()],
        &entries[0].layout,
        &ids(&["w2", "a2"]),
        &mut || "new-1".into(),
    );
    assert_eq!(
        groups
            .iter()
            .map(|group| group.id.as_str())
            .collect::<Vec<_>>(),
        vec!["g2", "new-1"]
    );
    assert_eq!(groups[1].active_tab_id, "a2");
    assert_eq!(
        groups[1].root,
        split(
            SplitOrientation::Horizontal,
            vec![pane_leaf("w2"), pane_leaf("a2")],
            vec![0.6, 0.4]
        )
    );
}

fn restored_pane(resize: bool) -> TabGroup {
    let entries = closed_entries(&all_tabs(), &[split_group()], &ids(&["api"]));
    let mut after = remove_tabs_from_groups(&[split_group()], &ids(&["api"]));
    if resize && let PaneNode::Split { sizes, .. } = &mut after[0].root {
        *sizes = vec![0.3, 0.7];
    }
    place_reopened_tabs(&after, &entries[0].layout, &ids(&["a2"]), &mut || {
        "new-2".into()
    })
    .remove(0)
}

#[test]
fn puts_a_pane_back_exactly_where_it_was_while_its_tab_is_as_it_was_left() {
    let group = restored_pane(false);
    assert_eq!(
        group.root,
        split(
            SplitOrientation::Horizontal,
            vec![
                pane_leaf("web"),
                split(
                    SplitOrientation::Vertical,
                    vec![pane_leaf("a2"), pane_leaf("logs")],
                    vec![0.5, 0.5]
                )
            ],
            vec![0.6, 0.4]
        )
    );
    assert_eq!(group.active_tab_id, "a2");
}

#[test]
fn puts_a_pane_beside_its_neighbour_once_its_tab_has_changed() {
    assert_eq!(
        restored_pane(true).root,
        split(
            SplitOrientation::Horizontal,
            vec![
                pane_leaf("web"),
                split(
                    SplitOrientation::Vertical,
                    vec![pane_leaf("a2"), pane_leaf("logs")],
                    vec![0.5, 0.5]
                )
            ],
            vec![0.3, 0.7]
        )
    );
}

#[test]
fn leaves_a_pane_whose_neighbour_is_gone_to_a_tab_of_its_own() {
    let entries = closed_entries(&all_tabs(), &[split_group()], &ids(&["api"]));
    assert_eq!(
        place_reopened_tabs(&[single()], &entries[0].layout, &ids(&["a2"]), &mut || {
            "new-3".into()
        }),
        vec![single()]
    );
}

#[test]
fn adds_the_closings_last_and_drops_the_oldest_past_the_limit() {
    let entries = (0..CLOSED_TABS_LIMIT)
        .flat_map(|index| {
            let id = format!("t{index}");
            closed_entries(
                &[tab(&id, json!({}))],
                &[TabGroup {
                    id: "g".into(),
                    root: pane_leaf(&id),
                    active_tab_id: id.clone(),
                    zoomed_tab_id: None,
                }],
                &[id],
            )
        })
        .collect::<Vec<_>>();
    let latest = closed_entries(
        &[tab("new", json!({}))],
        &[
            single(),
            TabGroup {
                id: "n".into(),
                root: pane_leaf("new"),
                active_tab_id: "new".into(),
                zoomed_tab_id: None,
            },
        ],
        &ids(&["new"]),
    );
    let closed = push_closed_entries(&entries, &latest);
    assert_eq!(closed.len(), CLOSED_TABS_LIMIT);
    assert_eq!(
        closed.last().unwrap().tabs[0].field("title"),
        Some(&json!("new"))
    );
    assert_eq!(closed[0].tabs[0].field("title"), Some(&json!("t1")));
}

#[test]
fn oracle_covers_closed_history_normal_and_empty_paths() {
    use crate::workspace::pane_templates::LayoutShape;
    use crate::workspace::tabs_state::reopen::reopened_active_tab_id;
    for case in super::oracle_11d2()["closedTabs"].as_array().unwrap() {
        let state = super::tabs_reducer::state(
            &json!({"tabs": case["tabs"], "groups": case["groups"], "activeTabId": null, "groupCount": 0}),
        );
        let ids = serde_json::from_value::<Vec<String>>(case["ids"].clone()).unwrap();
        let snapshots = state
            .tabs
            .iter()
            .map(closed_tab_snapshot)
            .collect::<Vec<_>>();
        assert_eq!(serde_json::to_value(snapshots).unwrap(), case["snapshots"]);
        let entries = closed_entries(&state.tabs, &state.groups, &ids);
        let empty_layout = ClosedLayout::Group {
            shape: LayoutShape::Pane,
            active_index: 0,
            index: 0,
        };
        assert_eq!(
            serde_json::to_value(place_reopened_tabs(
                &state.groups,
                &empty_layout,
                &[],
                &mut || "unused".into()
            ))
            .unwrap(),
            case["emptyPlacement"]
        );
        assert_eq!(
            serde_json::to_value(reopened_active_tab_id(&empty_layout, &[])).unwrap(),
            case["emptyActive"]
        );
        assert_eq!(serde_json::to_value(&entries).unwrap(), case["entries"]);
        assert_eq!(
            serde_json::to_value(push_closed_entries(&[], &entries)).unwrap(),
            case["pushed"]
        );
        for anchor in case["anchors"].as_array().unwrap() {
            assert_eq!(
                serde_json::to_value(pane_anchor(
                    &super::tabs_reducer::pane_node(&anchor["root"]),
                    anchor["id"].as_str().unwrap()
                ))
                .unwrap(),
                anchor["anchor"]
            );
        }
        for active in case["active"].as_array().unwrap() {
            let layout = ClosedLayout::Group {
                shape: LayoutShape::Pane,
                active_index: active["activeIndex"].as_i64().unwrap(),
                index: 0,
            };
            assert_eq!(
                serde_json::to_value(reopened_active_tab_id(&layout, &["x".into()])).unwrap(),
                active["id"]
            );
        }
    }
}
