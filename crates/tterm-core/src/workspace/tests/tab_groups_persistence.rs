use std::collections::HashSet;

use crate::workspace::tab_groups::{PaneNode, SplitOrientation, TabGroup, get_pane_ids, pane_leaf};
use crate::workspace::tab_groups_persistence::{
    RestoredTabGroup, SavedTabGroup, SavedTabGroups, restore_tab_groups, serialize_tab_groups,
};

use super::oracle_11d2;

fn pane(tab_id: &str) -> PaneNode {
    pane_leaf(tab_id)
}

fn row(children: Vec<PaneNode>) -> PaneNode {
    let count = children.len();
    PaneNode::Split {
        orientation: SplitOrientation::Horizontal,
        children,
        sizes: vec![1.0 / count as f64; count],
    }
}

fn row_sizes(children: Vec<PaneNode>, sizes: Vec<f64>) -> PaneNode {
    PaneNode::Split {
        orientation: SplitOrientation::Horizontal,
        children,
        sizes,
    }
}

fn saved(ids: &[&str]) -> HashSet<String> {
    ids.iter().map(|id| (*id).to_string()).collect()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn restored(root: PaneNode, active_tab_id: &str, zoomed_tab_id: Option<&str>) -> RestoredTabGroup {
    RestoredTabGroup {
        root,
        active_tab_id: active_tab_id.to_string(),
        zoomed_tab_id: zoomed_tab_id.map(str::to_string),
    }
}

fn constructed_pane(value: &serde_json::Value) -> PaneNode {
    match value["type"].as_str().expect("a node type") {
        "pane" => pane(value["tabId"].as_str().expect("a tab id")),
        "split" => {
            let orientation = match value["orientation"].as_str().expect("an orientation") {
                "horizontal" => SplitOrientation::Horizontal,
                "vertical" => SplitOrientation::Vertical,
                other => panic!("unknown orientation {other:?}"),
            };
            PaneNode::Split {
                orientation,
                children: value["children"]
                    .as_array()
                    .expect("children")
                    .iter()
                    .map(constructed_pane)
                    .collect(),
                // This section of the oracle uses null for the JavaScript
                // NaN and Infinity that JSON cannot carry.
                sizes: value["sizes"]
                    .as_array()
                    .expect("sizes")
                    .iter()
                    .map(|size| size.as_f64().unwrap_or(f64::NAN))
                    .collect(),
            }
        }
        other => panic!("unknown node type {other:?}"),
    }
}

fn constructed_group(value: &serde_json::Value) -> TabGroup {
    TabGroup {
        id: value["id"].as_str().expect("a group id").to_string(),
        root: constructed_pane(&value["root"]),
        active_tab_id: value["activeTabId"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        zoomed_tab_id: value["zoomedTabId"].as_str().map(str::to_string),
    }
}

#[test]
fn leaves_out_panes_whose_tabs_are_not_saved_and_groups_left_empty() {
    let groups = vec![
        TabGroup {
            id: "g1".to_string(),
            root: row(vec![pane("a"), pane("settings")]),
            active_tab_id: "settings".to_string(),
            zoomed_tab_id: None,
        },
        TabGroup {
            id: "g2".to_string(),
            root: pane("editor"),
            active_tab_id: "editor".to_string(),
            zoomed_tab_id: None,
        },
    ];
    assert_eq!(
        serialize_tab_groups(&groups, &saved(&["a"])),
        SavedTabGroups {
            version: 2,
            groups: vec![SavedTabGroup {
                root: pane("a"),
                active_tab_id: Some("a".to_string()),
                zoomed_tab_id: None,
            }],
        }
    );
}

#[test]
fn keeps_a_zoom_on_a_saved_pane() {
    let groups = vec![TabGroup {
        id: "g1".to_string(),
        root: row(vec![pane("a"), pane("b")]),
        active_tab_id: "b".to_string(),
        zoomed_tab_id: Some("b".to_string()),
    }];
    assert_eq!(
        serialize_tab_groups(&groups, &saved(&["a", "b"])).groups[0]
            .zoomed_tab_id
            .as_deref(),
        Some("b")
    );
}

#[test]
fn reads_back_what_was_saved_for_the_tabs_that_came_back() {
    let groups = vec![
        TabGroup {
            id: "g1".to_string(),
            root: row(vec![pane("a"), pane("b")]),
            active_tab_id: "b".to_string(),
            zoomed_tab_id: None,
        },
        TabGroup {
            id: "g2".to_string(),
            root: pane("c"),
            active_tab_id: "c".to_string(),
            zoomed_tab_id: None,
        },
    ];
    let saved_groups = serialize_tab_groups(&groups, &saved(&["a", "b", "c"]));
    let encoded = serde_json::to_string(&saved_groups).unwrap();
    let layout: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        restore_tab_groups(&layout, &strings(&["a", "c"])),
        vec![
            restored(pane("a"), "a", None),
            restored(pane("c"), "c", None),
        ]
    );
}

#[test]
fn skips_nodes_that_are_not_a_pane_tree() {
    let layout = serde_json::json!({
        "version": 2,
        "groups": [
            { "root": { "type": "split", "orientation": "diagonal", "children": [], "sizes": [] } },
            { "root": { "type": "pane", "tabId": 7 } },
            { "root": { "type": "pane", "tabId": "a" }, "activeTabId": "a" },
            "nonsense"
        ]
    });
    assert_eq!(
        restore_tab_groups(&layout, &strings(&["a"])),
        vec![restored(pane("a"), "a", None)]
    );
}

#[test]
fn restores_nothing_from_a_missing_or_broken_layout() {
    let tabs = strings(&["a"]);
    assert_eq!(
        restore_tab_groups(&serde_json::Value::Null, &tabs),
        Vec::<RestoredTabGroup>::new()
    );
    assert_eq!(
        restore_tab_groups(&serde_json::json!("layout"), &tabs),
        Vec::<RestoredTabGroup>::new()
    );
    assert_eq!(
        restore_tab_groups(&serde_json::json!({ "grid": {} }), &tabs),
        Vec::<RestoredTabGroup>::new()
    );
}

#[test]
fn turns_a_dockview_grid_into_one_split_of_the_tabs_in_front_the_rest_tabs_of_their_own() {
    let dockview = serde_json::json!({
        "grid": {
            "orientation": "HORIZONTAL",
            "root": {
                "type": "branch",
                "data": [
                    { "type": "leaf", "data": { "id": "1", "views": ["a", "b"], "activeView": "b" }, "size": 300 },
                    {
                        "type": "branch",
                        "size": 700,
                        "data": [
                            { "type": "leaf", "data": { "id": "2", "views": ["c"], "activeView": "c" }, "size": 100 },
                            { "type": "leaf", "data": { "id": "3", "views": ["d"], "activeView": "d" }, "size": 300 }
                        ]
                    }
                ]
            }
        },
        "panels": {},
        "activeGroup": "3"
    });
    let result = restore_tab_groups(&dockview, &strings(&["a", "b", "c", "d"]));
    assert_eq!(
        result
            .iter()
            .map(|candidate| get_pane_ids(&candidate.root))
            .collect::<Vec<_>>(),
        vec![strings(&["a"]), strings(&["b", "c", "d"]),]
    );
    let split = &result[1];
    assert_eq!(split.active_tab_id, "d");
    assert_eq!(
        split.root,
        row_sizes(
            vec![
                pane("b"),
                PaneNode::Split {
                    orientation: SplitOrientation::Vertical,
                    children: vec![pane("c"), pane("d")],
                    sizes: vec![0.25, 0.75],
                },
            ],
            vec![0.3, 0.7],
        )
    );
}

#[test]
fn oracle_round_trips_every_saved_layout_byte_for_byte() {
    let oracle = oracle_11d2();
    let cases = oracle["persistence"].as_array().expect("layout cases");
    assert!(!cases.is_empty());
    for case in cases {
        let name = case["name"].as_str().expect("a case name");
        let layout = &case["layout"];
        let tab_ids: Vec<String> = case["tabIds"]
            .as_array()
            .expect("tab ids")
            .iter()
            .map(|value| value.as_str().expect("a tab id").to_string())
            .collect();
        let restored = restore_tab_groups(layout, &tab_ids);
        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            case["restored"],
            "{name}: restored groups"
        );
        let groups: Vec<TabGroup> = restored
            .into_iter()
            .enumerate()
            .map(|(index, group)| TabGroup {
                id: format!("group-{}", index + 1),
                root: group.root,
                active_tab_id: group.active_tab_id,
                zoomed_tab_id: group.zoomed_tab_id,
            })
            .collect();
        let encoded = serde_json::to_string(&serialize_tab_groups(
            &groups,
            &tab_ids.iter().cloned().collect(),
        ))
        .unwrap();
        assert_eq!(encoded, case["bytes"].as_str().expect("bytes"), "{name}");
    }
}

#[test]
fn oracle_treats_only_numeric_version_two_as_the_current_format() {
    let oracle = oracle_11d2();
    let cases = oracle["persistence"].as_array().expect("layout cases");
    for name in [
        "version 2.0",
        "version string 2 is Dockview",
        "old version is Dockview",
        "missing version",
        "real old resume sample",
    ] {
        let case = cases
            .iter()
            .find(|case| case["name"] == name)
            .unwrap_or_else(|| panic!("oracle case {name}"));
        let tab_ids: Vec<String> = case["tabIds"]
            .as_array()
            .expect("tab ids")
            .iter()
            .map(|value| value.as_str().expect("a tab id").to_string())
            .collect();
        let restored = restore_tab_groups(&case["layout"], &tab_ids);
        assert_eq!(
            serde_json::to_value(restored).unwrap(),
            case["restored"],
            "{name}"
        );
    }
}

#[test]
fn oracle_serializes_constructed_groups_byte_for_byte() {
    let oracle = oracle_11d2();
    let cases = oracle["serializeTabGroups"]
        .as_array()
        .expect("serialization cases");
    assert!(!cases.is_empty());
    for case in cases {
        let groups: Vec<TabGroup> = case["groups"]
            .as_array()
            .expect("groups")
            .iter()
            .map(constructed_group)
            .collect();
        let saved: HashSet<String> = case["savedTabIds"]
            .as_array()
            .expect("saved tab ids")
            .iter()
            .map(|value| value.as_str().expect("a tab id").to_string())
            .collect();
        let serialized = serialize_tab_groups(&groups, &saved);
        assert_eq!(
            serde_json::to_value(&serialized).unwrap(),
            case["value"],
            "{}",
            case["name"]
        );
        assert_eq!(
            serde_json::to_string(&serialized).unwrap(),
            case["bytes"].as_str().expect("bytes"),
            "{}",
            case["name"]
        );
    }
}
