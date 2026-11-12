//! Split pane trees and groups, from `src/lib/tabGroups.ts`.
//!
//! This is the subset the pane layout, navigation and template modules use.
//! The second P1-11d group completes the remaining tab group operations here.

use serde::{Deserialize, Serialize};

use super::js_number::serialize_sizes;

/// Whether a split lays its children side by side or stacks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitOrientation {
    /// Children sit side by side from left to right.
    Horizontal,
    /// Children stack from top to bottom.
    Vertical,
}

/// A node of a tab's pane tree.
///
/// Saved sessions serialize this type. Reading saved trees is deferred to the
/// second P1-11d group, whose `readPaneNode` accepts malformed sizes and drops
/// only the invalid group; deriving `Deserialize` here would be stricter than
/// the TypeScript format.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PaneNode {
    /// One pane showing the tab with `tab_id`.
    Pane {
        /// The tab the pane shows.
        #[serde(rename = "tabId")]
        tab_id: String,
    },
    /// A split whose children own the shares in `sizes`.
    Split {
        /// The axis the children divide.
        orientation: SplitOrientation,
        /// The panes or splits that make up the split.
        children: Vec<PaneNode>,
        /// Each child's share of the split.
        #[serde(serialize_with = "serialize_sizes")]
        sizes: Vec<f64>,
    },
}

/// One tab of the tab bar and the panes it holds.
///
/// The saved session format serializes this type. Reading it belongs to the
/// second P1-11d group with [`PaneNode`]'s reader.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabGroup {
    /// The group's unique id.
    pub id: String,
    /// The group's split tree.
    pub root: PaneNode,
    /// The pane focused last.
    pub active_tab_id: String,
    /// A pane filling the group while the split behind it stays as it was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoomed_tab_id: Option<String>,
}

/// A leaf pane showing `tab_id`.
pub fn pane_leaf(tab_id: impl Into<String>) -> PaneNode {
    PaneNode::Pane {
        tab_id: tab_id.into(),
    }
}

/// The tab ids of a tree's panes, left to right and top to bottom.
pub fn get_pane_ids(node: &PaneNode) -> Vec<String> {
    match node {
        PaneNode::Pane { tab_id } => vec![tab_id.clone()],
        PaneNode::Split { children, .. } => children.iter().flat_map(get_pane_ids).collect(),
    }
}

/// The first group holding `tab_id`.
pub fn find_group_of_tab<'a>(groups: &'a [TabGroup], tab_id: &str) -> Option<&'a TabGroup> {
    groups
        .iter()
        .find(|group| get_pane_ids(&group.root).iter().any(|id| id == tab_id))
}

fn scale_to_one(sizes: &[f64]) -> Vec<f64> {
    let total: f64 = sizes
        .iter()
        .map(|size| if *size > 0.0 { *size } else { 0.0 })
        .sum();
    if total <= 0.0 {
        return sizes.iter().map(|_| 1.0 / sizes.len() as f64).collect();
    }
    sizes
        .iter()
        .map(|size| if *size > 0.0 { *size } else { 0.0 } / total)
        .collect()
}

/// Puts a tree into its canonical shape: one-child splits collapse, splits of
/// the same orientation join, and every split's sizes sum to one.
pub fn normalize_pane_tree(node: &PaneNode) -> PaneNode {
    let PaneNode::Split {
        orientation,
        children,
        sizes,
    } = node
    else {
        return node.clone();
    };
    let own_sizes = scale_to_one(
        &children
            .iter()
            .enumerate()
            .map(|(index, _)| {
                sizes
                    .get(index)
                    .copied()
                    .unwrap_or(1.0 / children.len() as f64)
            })
            .collect::<Vec<_>>(),
    );
    let mut normalized_children = Vec::new();
    let mut normalized_sizes = Vec::new();
    for (index, child) in children.iter().enumerate() {
        let normalized = normalize_pane_tree(child);
        match normalized {
            PaneNode::Split {
                orientation: child_orientation,
                children: child_children,
                sizes: child_sizes,
            } if child_orientation == *orientation => {
                normalized_sizes.extend(child_sizes.iter().map(|size| size * own_sizes[index]));
                normalized_children.extend(child_children);
            }
            other => {
                normalized_children.push(other);
                normalized_sizes.push(own_sizes[index]);
            }
        }
    }
    if normalized_children.len() == 1 {
        return normalized_children.remove(0);
    }
    PaneNode::Split {
        orientation: *orientation,
        children: normalized_children,
        sizes: scale_to_one(&normalized_sizes),
    }
}
