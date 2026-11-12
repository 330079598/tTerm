//! Saved tab group layouts, from `src/lib/tabGroupsPersistence.ts`.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::{Map, Value};

use super::tab_groups::{
    PaneNode, SplitOrientation, TabGroup, get_pane_ids, normalize_pane_tree, pane_leaf,
    remove_panes,
};

/// The layout version current tTerm writes.
pub const TAB_GROUPS_LAYOUT_VERSION: u8 = 2;

/// One group as saved in a session.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedTabGroup {
    /// The group's split tree.
    pub root: PaneNode,
    /// The pane focused last; a group with no pane has none, matching the
    /// key `JSON.stringify` leaves out for `undefined`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_tab_id: Option<String>,
    /// A pane filling the group while the split behind it stays as it was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoomed_tab_id: Option<String>,
}

/// The tab groups saved in a session.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SavedTabGroups {
    /// The format version, [`TAB_GROUPS_LAYOUT_VERSION`].
    pub version: u8,
    /// The saved groups in tab-bar order.
    pub groups: Vec<SavedTabGroup>,
}

/// A restored group before the tab state gives it an id.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoredTabGroup {
    /// The group's split tree.
    pub root: PaneNode,
    /// The pane focused last.
    pub active_tab_id: String,
    /// A pane filling the group while the split behind it stays as it was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoomed_tab_id: Option<String>,
}

/// The groups to save, holding only the panes whose tabs are saved too.
pub fn serialize_tab_groups(
    groups: &[TabGroup],
    saved_tab_ids: &HashSet<String>,
) -> SavedTabGroups {
    let saved = groups
        .iter()
        .flat_map(|group| {
            let unsaved: Vec<String> = get_pane_ids(&group.root)
                .into_iter()
                .filter(|id| !saved_tab_ids.contains(id))
                .collect();
            let root = if unsaved.is_empty() {
                Some(group.root.clone())
            } else {
                remove_panes(&group.root, &unsaved)
            }?;
            let pane_ids = get_pane_ids(&root);
            let active_tab_id = if pane_ids.contains(&group.active_tab_id) {
                Some(group.active_tab_id.clone())
            } else {
                pane_ids.first().cloned()
            };
            let zoomed_tab_id = group
                .zoomed_tab_id
                .clone()
                .filter(|id| !id.is_empty() && pane_ids.contains(id));
            Some(SavedTabGroup {
                root,
                active_tab_id,
                zoomed_tab_id,
            })
        })
        .collect();
    SavedTabGroups {
        version: TAB_GROUPS_LAYOUT_VERSION,
        groups: saved,
    }
}

fn read_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

fn read_pane_node(value: &Value) -> Option<PaneNode> {
    let object = value.as_object()?;
    match object.get("type").and_then(Value::as_str) {
        Some("pane") => object.get("tabId").and_then(Value::as_str).map(pane_leaf),
        Some("split") => {
            let orientation = match object.get("orientation").and_then(Value::as_str) {
                Some("horizontal") => SplitOrientation::Horizontal,
                Some("vertical") => SplitOrientation::Vertical,
                _ => return None,
            };
            let children = object.get("children")?.as_array()?;
            let raw_sizes = object.get("sizes")?.as_array()?;
            if children.is_empty() {
                return None;
            }
            let mut nodes = Vec::with_capacity(children.len());
            for child in children {
                nodes.push(read_pane_node(child)?);
            }
            let sizes = nodes
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    raw_sizes
                        .get(index)
                        .and_then(Value::as_f64)
                        .filter(|size| *size > 0.0)
                        .unwrap_or(0.0)
                })
                .collect();
            Some(PaneNode::Split {
                orientation,
                children: nodes,
                sizes,
            })
        }
        _ => None,
    }
}

fn read_saved_groups(value: &Map<String, Value>) -> Vec<SavedTabGroup> {
    let Some(groups) = value.get("groups").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut saved = Vec::new();
    for group in groups {
        let Some(group) = group.as_object() else {
            continue;
        };
        let Some(root) = group.get("root").and_then(read_pane_node) else {
            continue;
        };
        saved.push(SavedTabGroup {
            root,
            active_tab_id: Some(
                group
                    .get("activeTabId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
            zoomed_tab_id: read_string(group.get("zoomedTabId")),
        });
    }
    saved
}

fn read_dockview_layout(value: &Map<String, Value>) -> Vec<SavedTabGroup> {
    let Some(grid) = value.get("grid").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut behind = Vec::new();
    let mut front_by_group_id: HashMap<String, String> = HashMap::new();

    fn visit(
        node: &Value,
        orientation: SplitOrientation,
        behind: &mut Vec<String>,
        front_by_group_id: &mut HashMap<String, String>,
    ) -> Option<PaneNode> {
        let object = node.as_object()?;
        match object.get("type").and_then(Value::as_str) {
            Some("leaf") => {
                let data = object.get("data").and_then(Value::as_object);
                let views: Vec<&str> = data
                    .and_then(|data| data.get("views"))
                    .and_then(Value::as_array)
                    .map(|views| views.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                let active_view = data
                    .and_then(|data| data.get("activeView"))
                    .and_then(Value::as_str);
                let front = active_view
                    .filter(|active| views.contains(active))
                    .or_else(|| views.first().copied())
                    .filter(|front| !front.is_empty())?;
                if let Some(id) = data.and_then(|data| data.get("id")).and_then(Value::as_str) {
                    front_by_group_id.insert(id.to_string(), front.to_string());
                }
                behind.extend(
                    views
                        .iter()
                        .filter(|view| **view != front)
                        .map(|view| (*view).to_string()),
                );
                Some(pane_leaf(front))
            }
            Some("branch") => {
                let children = object.get("data").and_then(Value::as_array)?;
                let crossed = match orientation {
                    SplitOrientation::Horizontal => SplitOrientation::Vertical,
                    SplitOrientation::Vertical => SplitOrientation::Horizontal,
                };
                let mut panes = Vec::new();
                let mut sizes = Vec::new();
                for child in children {
                    let pane = visit(child, crossed, behind, front_by_group_id);
                    let size = child
                        .as_object()
                        .and_then(|child| child.get("size"))
                        .and_then(Value::as_f64)
                        .unwrap_or(1.0);
                    if let Some(pane) = pane {
                        panes.push(pane);
                        sizes.push(size);
                    }
                }
                if panes.is_empty() {
                    return None;
                }
                Some(PaneNode::Split {
                    orientation,
                    children: panes,
                    sizes,
                })
            }
            _ => None,
        }
    }

    let root_orientation = if grid.get("orientation").and_then(Value::as_str) == Some("VERTICAL") {
        SplitOrientation::Vertical
    } else {
        SplitOrientation::Horizontal
    };
    let Some(root) = grid
        .get("root")
        .and_then(|root| visit(root, root_orientation, &mut behind, &mut front_by_group_id))
    else {
        return Vec::new();
    };
    let front = normalize_pane_tree(&root);
    let active_front = value
        .get("activeGroup")
        .and_then(Value::as_str)
        .and_then(|group_id| front_by_group_id.get(group_id))
        .cloned()
        .or_else(|| get_pane_ids(&front).first().cloned())
        .unwrap_or_default();
    let mut groups = vec![SavedTabGroup {
        root: front,
        active_tab_id: Some(active_front),
        zoomed_tab_id: None,
    }];
    groups.extend(behind.into_iter().map(|tab_id| SavedTabGroup {
        root: pane_leaf(tab_id.clone()),
        active_tab_id: Some(tab_id),
        zoomed_tab_id: None,
    }));
    groups
}

/// The groups of a saved layout, for the tabs restored with it.
///
/// Panes whose tab was not restored are left out; tabs no group holds are
/// left for the caller to give groups of their own. A layout saved by
/// Dockview keeps the tabs' order from the tab list.
pub fn restore_tab_groups(layout: &Value, tab_ids: &[String]) -> Vec<RestoredTabGroup> {
    let Some(value) = layout.as_object() else {
        return Vec::new();
    };
    let is_dockview = value.get("version").and_then(Value::as_f64) != Some(2.0);
    let saved = if is_dockview {
        read_dockview_layout(value)
    } else {
        read_saved_groups(value)
    };
    let known: HashSet<&str> = tab_ids.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    let mut groups = Vec::new();
    for group in saved {
        let dropped: Vec<String> = get_pane_ids(&group.root)
            .into_iter()
            .filter(|id| !known.contains(id.as_str()) || seen.contains(id))
            .collect();
        let root = if dropped.is_empty() {
            Some(group.root)
        } else {
            remove_panes(&group.root, &dropped)
        };
        let Some(root) = root else {
            continue;
        };
        let pane_ids = get_pane_ids(&root);
        for id in &pane_ids {
            seen.insert(id.clone());
        }
        let active_tab_id = group
            .active_tab_id
            .filter(|active| pane_ids.contains(active))
            .or_else(|| pane_ids.first().cloned())
            .unwrap_or_default();
        let zoomed_tab_id = group
            .zoomed_tab_id
            .filter(|id| !id.is_empty() && pane_ids.contains(id));
        groups.push(RestoredTabGroup {
            root: normalize_pane_tree(&root),
            active_tab_id,
            zoomed_tab_id,
        });
    }
    if !is_dockview {
        return groups;
    }
    groups.sort_by_key(|group| {
        get_pane_ids(&group.root)
            .iter()
            .map(|id| {
                tab_ids
                    .iter()
                    .position(|tab_id| tab_id == id)
                    .map_or(-1, |index| index as isize)
            })
            .min()
            .unwrap_or(-1)
    });
    groups
}
