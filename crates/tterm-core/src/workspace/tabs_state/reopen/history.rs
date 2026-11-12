//! Snapshots and bounded close history consumed by the reopen action.

use super::{ClosedLayout, PaneAnchor};
use crate::workspace::pane_templates::shape_of_tree;
use crate::workspace::tab::{Tab, is_session_tab};
use crate::workspace::tab_groups::{
    PaneNode, SplitDirection, SplitOrientation, TabGroup, get_pane_ids, remove_panes,
};
use serde_json::{Map, Value};

/// The maximum number of closings retained, oldest to newest.
pub const CLOSED_TABS_LIMIT: usize = 20;

/// A session snapshot without an allocated id or active flag.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ClosedTab(Map<String, Value>);

impl ClosedTab {
    /// Reads a snapshot field without exposing mutable duplicate state.
    pub fn field(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }
}

/// One closing and the group or pane position it can be reopened at.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ClosedEntry {
    /// Session snapshots in pane order.
    pub tabs: Vec<ClosedTab>,
    /// Their position when they closed.
    pub layout: ClosedLayout,
}

/// Keeps session settings but removes live state and typed passwords.
pub fn closed_tab_snapshot(tab: &Tab) -> Option<ClosedTab> {
    if !is_session_tab(tab) {
        return None;
    }
    let Value::Object(mut fields) = serde_json::to_value(tab).ok()? else {
        return None;
    };
    for key in [
        "id",
        "isActive",
        "pid",
        "hasConnected",
        "sessionNonce",
        "quickConnectAuth",
    ] {
        fields.shift_remove(key);
    }
    if let Some(Value::Object(connection)) = fields.get_mut("connection") {
        connection.shift_remove("password");
        connection.shift_remove("privateKeyPassphrase");
        if let Some(Value::Array(jumps)) = connection.get_mut("jumpHosts") {
            for jump in jumps {
                if let Value::Object(jump) = jump {
                    jump.shift_remove("password");
                    jump.shift_remove("privateKeyPassphrase");
                }
            }
        }
    }
    fields.insert("quickConnect".into(), false.into());
    Some(ClosedTab(fields))
}

/// Finds the preceding pane, or the next pane when closing the first child.
pub fn pane_anchor(root: &PaneNode, tab_id: &str) -> Option<PaneAnchor> {
    let PaneNode::Split {
        orientation,
        children,
        ..
    } = root
    else {
        return None;
    };
    let Some(index) = children
        .iter()
        .position(|child| matches!(child, PaneNode::Pane { tab_id: id } if id == tab_id))
    else {
        return children.iter().find_map(|child| pane_anchor(child, tab_id));
    };
    let horizontal = *orientation == SplitOrientation::Horizontal;
    if index > 0 {
        Some(PaneAnchor {
            tab_id: get_pane_ids(&children[index - 1]).last()?.clone(),
            direction: if horizontal {
                SplitDirection::Right
            } else {
                SplitDirection::Below
            },
        })
    } else {
        Some(PaneAnchor {
            tab_id: get_pane_ids(children.get(1)?).first()?.clone(),
            direction: if horizontal {
                SplitDirection::Left
            } else {
                SplitDirection::Above
            },
        })
    }
}

fn closed_group(group: &TabGroup, index: usize, tabs: &[Tab]) -> Option<ClosedEntry> {
    let mut snapshots = indexmap::IndexMap::new();
    for id in get_pane_ids(&group.root) {
        if let Some(snapshot) = tabs
            .iter()
            .rev()
            .find(|tab| tab.id == id)
            .and_then(closed_tab_snapshot)
        {
            snapshots.insert(id, snapshot);
        }
    }
    let dropped = get_pane_ids(&group.root)
        .into_iter()
        .filter(|id| !snapshots.contains_key(id))
        .collect::<Vec<_>>();
    let root = remove_panes(&group.root, &dropped)?;
    let order = get_pane_ids(&root);
    Some(ClosedEntry {
        tabs: order
            .iter()
            .filter_map(|id| snapshots.get(id).cloned())
            .collect(),
        layout: ClosedLayout::Group {
            shape: shape_of_tree(&root),
            active_index: order
                .iter()
                .position(|id| *id == group.active_tab_id)
                .unwrap_or(0) as i64,
            index: index as i64,
        },
    })
}

/// Records each closed group or individually closed pane in tab-bar order.
pub fn closed_entries(tabs: &[Tab], groups: &[TabGroup], ids: &[String]) -> Vec<ClosedEntry> {
    let mut entries = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let panes = get_pane_ids(&group.root);
        let closed = panes
            .iter()
            .filter(|id| ids.contains(id))
            .collect::<Vec<_>>();
        if closed.is_empty() {
            continue;
        }
        if closed.len() == panes.len() {
            if let Some(entry) = closed_group(group, index, tabs) {
                entries.push(entry);
            }
        } else {
            for id in closed {
                if let Some(snapshot) = tabs
                    .iter()
                    .rev()
                    .find(|tab| tab.id == *id)
                    .and_then(closed_tab_snapshot)
                {
                    entries.push(ClosedEntry {
                        tabs: vec![snapshot],
                        layout: ClosedLayout::Pane {
                            closed_id: id.clone(),
                            previous_root: group.root.clone(),
                            anchor: pane_anchor(&group.root, id),
                        },
                    });
                }
            }
        }
    }
    entries
}

/// Appends the latest closings and drops the oldest past the history limit.
pub fn push_closed_entries(closed: &[ClosedEntry], entries: &[ClosedEntry]) -> Vec<ClosedEntry> {
    let count = closed.len().saturating_add(entries.len());
    closed
        .iter()
        .chain(entries)
        .skip(count.saturating_sub(CLOSED_TABS_LIMIT))
        .cloned()
        .collect()
}
