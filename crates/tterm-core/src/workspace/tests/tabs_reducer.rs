use serde_json::Value;

use crate::workspace::tab_groups::{PaneNode, SplitDirection, TabGroup};
use crate::workspace::tabs_state::action::TabsAction;
use crate::workspace::tabs_state::reopen::{ClosedLayout, PaneAnchor};
use crate::workspace::tabs_state::{RemoveTabsOptions, TabsState, tabs_reducer};

use super::oracle_11d2;

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().expect(key).to_string()
}

fn strings(value: &Value) -> Vec<String> {
    serde_json::from_value(value.clone()).unwrap()
}

pub(super) fn pane_node(value: &Value) -> PaneNode {
    if value["type"] == "pane" {
        PaneNode::Pane {
            tab_id: text(value, "tabId"),
        }
    } else {
        PaneNode::Split {
            orientation: serde_json::from_value(value["orientation"].clone()).unwrap(),
            children: value["children"]
                .as_array()
                .unwrap()
                .iter()
                .map(pane_node)
                .collect(),
            sizes: serde_json::from_value(value["sizes"].clone()).unwrap(),
        }
    }
}

fn state(value: &Value) -> TabsState {
    TabsState {
        tabs: serde_json::from_value(value["tabs"].clone()).unwrap(),
        active_tab_id: value["activeTabId"].as_str().map(str::to_string),
        groups: value["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| TabGroup {
                id: text(group, "id"),
                root: pane_node(&group["root"]),
                active_tab_id: text(group, "activeTabId"),
                zoomed_tab_id: group["zoomedTabId"].as_str().map(str::to_string),
            })
            .collect(),
        group_count: value["groupCount"].as_u64().unwrap(),
    }
}

pub(super) fn direction(value: &Value) -> SplitDirection {
    match value.as_str().unwrap() {
        "left" => SplitDirection::Left,
        "right" => SplitDirection::Right,
        "above" => SplitDirection::Above,
        "below" => SplitDirection::Below,
        other => panic!("unknown direction {other}"),
    }
}

fn layout(value: &Value) -> ClosedLayout {
    if value["kind"] == "group" {
        ClosedLayout::Group {
            shape: serde_json::from_value(value["shape"].clone()).unwrap(),
            active_index: value["activeIndex"].as_i64().unwrap(),
            index: value["index"].as_i64().unwrap(),
        }
    } else {
        ClosedLayout::Pane {
            closed_id: text(value, "closedId"),
            previous_root: pane_node(&value["previousRoot"]),
            anchor: value["anchor"].as_object().map(|_| PaneAnchor {
                tab_id: text(&value["anchor"], "tabId"),
                direction: direction(&value["anchor"]["direction"]),
            }),
        }
    }
}

fn action(value: &Value) -> TabsAction {
    match value["type"].as_str().unwrap() {
        "restore" => TabsAction::Restore {
            tabs: serde_json::from_value(value["tabs"].clone()).unwrap(),
            active_tab_id: value["activeTabId"].as_str().map(str::to_string),
            layout: value["layout"].clone(),
        },
        "add" => TabsAction::Add(serde_json::from_value(value["tab"].clone()).unwrap()),
        "open-page" => TabsAction::OpenPage(serde_json::from_value(value["tab"].clone()).unwrap()),
        "rename-page" => TabsAction::RenamePage {
            tab_type: serde_json::from_value(value["tabType"].clone()).unwrap(),
            title: text(value, "title"),
        },
        "remove" => TabsAction::Remove {
            ids: strings(&value["ids"]),
            options: RemoveTabsOptions {
                preferred_active_tab_id: value["options"]["preferredActiveTabId"]
                    .as_str()
                    .map(str::to_string),
                activate_preferred: value["options"]["activatePreferred"]
                    .as_bool()
                    .unwrap_or(false),
            },
        },
        "set-active" => TabsAction::SetActive(text(value, "id")),
        "move" => TabsAction::Move {
            from_index: value["fromIndex"].as_i64().unwrap(),
            to_index: value["toIndex"].as_i64().unwrap(),
        },
        "duplicate" => TabsAction::Duplicate {
            source_id: text(value, "sourceId"),
            new_id: text(value, "newId"),
            split: value.get("split").map(direction),
        },
        "rename" => TabsAction::Rename {
            id: text(value, "id"),
            title: text(value, "title"),
        },
        "update" => {
            let patch = value["patch"].as_object().unwrap().clone();
            TabsAction::Update {
                id: text(value, "id"),
                updater: Box::new(move |tab| {
                    let mut value = serde_json::to_value(tab).unwrap();
                    value.as_object_mut().unwrap().extend(patch.clone());
                    serde_json::from_value(value).unwrap()
                }),
            }
        }
        "place-pane" => TabsAction::PlacePane {
            id: text(value, "id"),
            target_id: text(value, "targetId"),
            direction: direction(&value["direction"]),
        },
        "merge-group" => TabsAction::MergeGroup {
            id: text(value, "id"),
            target_id: text(value, "targetId"),
            direction: direction(&value["direction"]),
        },
        "break-out" => TabsAction::BreakOut {
            id: text(value, "id"),
            index: value["index"].as_i64(),
        },
        "ungroup" => TabsAction::Ungroup(text(value, "id")),
        "toggle-zoom" => TabsAction::ToggleZoom(text(value, "id")),
        "resize-split" => TabsAction::ResizeSplit {
            group_id: text(value, "groupId"),
            path: serde_json::from_value(value["path"].clone()).unwrap(),
            sizes: serde_json::from_value(value["sizes"].clone()).unwrap(),
        },
        "apply-template" => TabsAction::ApplyTemplate {
            id: text(value, "id"),
            root: serde_json::from_value(value["root"].clone()).unwrap(),
            new_ids: strings(&value["newIds"]),
        },
        "reopen" => TabsAction::Reopen {
            tabs: serde_json::from_value(value["tabs"].clone()).unwrap(),
            layout: layout(&value["layout"]),
        },
        other => panic!("unknown action {other}"),
    }
}

fn check_action(action_type: &str) {
    let oracle = oracle_11d2();
    for case in oracle["reducer"]
        .as_array()
        .expect("reducer cases")
        .iter()
        .chain(
            oracle["reducerWithHome"]
                .as_array()
                .expect("loaded home cases"),
        )
    {
        if case["action"]["type"] != action_type { continue; }
        let result = tabs_reducer(
            state(&case["state"]),
            action(&case["action"]),
            case["home"].as_str(),
        );
        assert_eq!(
            serde_json::to_string(&result).unwrap(),
            case["resultBytes"].as_str().unwrap(),
            "{}",
            case["name"]
        );
        assert_eq!(
            serde_json::to_string(&result.tabs).unwrap(),
            case["tabBytes"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}

macro_rules! action_tests {
    ($($name:ident => $kind:literal),* $(,)?) => {$(
        #[test]
        fn $name() { check_action($kind); }
    )*};
}

action_tests! {
    oracle_restore => "restore",
    oracle_add => "add",
    oracle_open_page => "open-page",
    oracle_rename_page => "rename-page",
    oracle_remove => "remove",
    oracle_set_active => "set-active",
    oracle_move => "move",
    oracle_duplicate => "duplicate",
    oracle_rename => "rename",
    oracle_update => "update",
    oracle_place_pane => "place-pane",
    oracle_merge_group => "merge-group",
    oracle_break_out => "break-out",
    oracle_ungroup => "ungroup",
    oracle_toggle_zoom => "toggle-zoom",
    oracle_resize_split => "resize-split",
    oracle_apply_template => "apply-template",
    oracle_reopen => "reopen",
}
