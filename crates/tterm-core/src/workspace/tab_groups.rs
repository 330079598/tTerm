//! Split pane trees and groups, from `src/lib/tabGroups.ts`.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::js_number::serialize_sizes;

/// The side of a pane on which a new pane or group is inserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Insert to the left.
    Left,
    /// Insert to the right.
    Right,
    /// Insert above.
    Above,
    /// Insert below.
    Below,
}

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
/// Saved sessions serialize this type. Reading saved trees goes through the
/// lenient `read_pane_node` in [`super::tab_groups_persistence`], which accepts
/// malformed sizes and drops only the invalid group.
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
/// The saved session format serializes this type; [`super::tab_groups_persistence`]
/// reads it back leniently.
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

/// The orientation a split in `direction` uses.
pub fn split_orientation(direction: SplitDirection) -> SplitOrientation {
    match direction {
        SplitDirection::Left | SplitDirection::Right => SplitOrientation::Horizontal,
        SplitDirection::Above | SplitDirection::Below => SplitOrientation::Vertical,
    }
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

fn insert_into(
    node: &PaneNode,
    target_tab_id: &str,
    inserted: &PaneNode,
    direction: SplitDirection,
) -> PaneNode {
    let orientation = split_orientation(direction);
    let before = matches!(direction, SplitDirection::Left | SplitDirection::Above);
    match node {
        PaneNode::Pane { tab_id } => {
            if tab_id != target_tab_id {
                return node.clone();
            }
            let children = if before {
                vec![inserted.clone(), node.clone()]
            } else {
                vec![node.clone(), inserted.clone()]
            };
            PaneNode::Split {
                orientation,
                children,
                sizes: vec![0.5, 0.5],
            }
        }
        PaneNode::Split {
            orientation: node_orientation,
            children,
            sizes,
        } => {
            let index = children.iter().position(
                |child| matches!(child, PaneNode::Pane { tab_id } if tab_id == target_tab_id),
            );
            if let Some(index) = index.filter(|_| *node_orientation == orientation) {
                let half = sizes.get(index).copied().unwrap_or(f64::NAN) / 2.0;
                let at = if before { index } else { index + 1 };
                let mut next_children = children.clone();
                next_children.insert(at, inserted.clone());
                let mut next_sizes = sizes.clone();
                if let Some(size) = next_sizes.get_mut(index) {
                    *size = half;
                }
                next_sizes.insert(at, half);
                return PaneNode::Split {
                    orientation: *node_orientation,
                    children: next_children,
                    sizes: next_sizes,
                };
            }
            PaneNode::Split {
                orientation: *node_orientation,
                children: children
                    .iter()
                    .map(|child| insert_into(child, target_tab_id, inserted, direction))
                    .collect(),
                sizes: sizes.clone(),
            }
        }
    }
}

/// Puts a pane, or a whole tree, beside the target pane, which gives up half
/// its room.
pub fn insert_next_to(
    root: &PaneNode,
    target_tab_id: &str,
    inserted: &PaneNode,
    direction: SplitDirection,
) -> PaneNode {
    normalize_pane_tree(&insert_into(root, target_tab_id, inserted, direction))
}

fn remove_from(node: &PaneNode, tab_ids: &HashSet<String>) -> Option<PaneNode> {
    match node {
        PaneNode::Pane { tab_id } => {
            if tab_ids.contains(tab_id) {
                None
            } else {
                Some(node.clone())
            }
        }
        PaneNode::Split {
            orientation,
            children,
            sizes,
        } => {
            let mut next_children = Vec::new();
            let mut next_sizes = Vec::new();
            for (index, child) in children.iter().enumerate() {
                if let Some(kept) = remove_from(child, tab_ids) {
                    next_children.push(kept);
                    next_sizes.push(sizes.get(index).copied().unwrap_or(f64::NAN));
                }
            }
            if next_children.is_empty() {
                None
            } else {
                Some(PaneNode::Split {
                    orientation: *orientation,
                    children: next_children,
                    sizes: scale_to_one(&next_sizes),
                })
            }
        }
    }
}

/// The tree without the given panes, or `None` when none is left.
pub fn remove_panes(root: &PaneNode, tab_ids: &[String]) -> Option<PaneNode> {
    let removed = remove_from(root, &tab_ids.iter().cloned().collect())?;
    Some(normalize_pane_tree(&removed))
}

/// Sets the sizes of the split found by following `path`, a child index per
/// level.
pub fn resize_split(root: &PaneNode, path: &[i64], sizes: &[f64]) -> PaneNode {
    let PaneNode::Split {
        orientation,
        children,
        sizes: current_sizes,
    } = root
    else {
        return root.clone();
    };
    let Some((index, rest)) = path.split_first() else {
        return if children.len() == sizes.len() {
            PaneNode::Split {
                orientation: *orientation,
                children: children.clone(),
                sizes: scale_to_one(sizes),
            }
        } else {
            root.clone()
        };
    };
    if *index < 0 {
        return root.clone();
    }
    let Some(child) = children.get(*index as usize) else {
        return root.clone();
    };
    let mut next_children = children.clone();
    next_children[*index as usize] = resize_split(child, rest, sizes);
    PaneNode::Split {
        orientation: *orientation,
        children: next_children,
        sizes: current_sizes.clone(),
    }
}

fn nearest_pane(order: &[String], tab_id: &str, remaining: &[String]) -> String {
    let origin = order
        .iter()
        .position(|id| id == tab_id)
        .map_or(-1, |index| index as isize);
    let kept: HashSet<&str> = remaining.iter().map(String::as_str).collect();
    for offset in 1..order.len() as isize {
        let after = origin + offset;
        if after >= 0
            && let Some(candidate) = order.get(after as usize)
            && kept.contains(candidate.as_str())
        {
            return candidate.clone();
        }
        let before = origin - offset;
        if before >= 0
            && let Some(candidate) = order.get(before as usize)
            && kept.contains(candidate.as_str())
        {
            return candidate.clone();
        }
    }
    remaining.first().cloned().unwrap_or_default()
}

/// The groups without the given panes; a group left with no pane goes.
pub fn remove_tabs_from_groups(groups: &[TabGroup], tab_ids: &[String]) -> Vec<TabGroup> {
    let ids: HashSet<&str> = tab_ids.iter().map(String::as_str).collect();
    if !groups.iter().any(|group| {
        get_pane_ids(&group.root)
            .iter()
            .any(|id| ids.contains(id.as_str()))
    }) {
        return groups.to_vec();
    }
    let mut result = Vec::new();
    for group in groups {
        let order = get_pane_ids(&group.root);
        if !order.iter().any(|id| ids.contains(id.as_str())) {
            result.push(group.clone());
            continue;
        }
        let Some(root) = remove_panes(&group.root, tab_ids) else {
            continue;
        };
        let remaining = get_pane_ids(&root);
        let active_tab_id = if remaining.contains(&group.active_tab_id) {
            group.active_tab_id.clone()
        } else {
            nearest_pane(&order, &group.active_tab_id, &remaining)
        };
        let zoomed_tab_id = group
            .zoomed_tab_id
            .as_ref()
            .filter(|zoomed| !zoomed.is_empty() && remaining.contains(zoomed))
            .cloned();
        result.push(TabGroup {
            id: group.id.clone(),
            root,
            active_tab_id,
            zoomed_tab_id,
        });
    }
    result
}

/// The pane to focus once the active one is gone: another pane of its group,
/// or else the group that took its group's place in the bar, or the last one.
pub fn get_fallback_tab_id(
    previous: &[TabGroup],
    next: &[TabGroup],
    active_tab_id: Option<&str>,
) -> Option<String> {
    if next.is_empty() {
        return None;
    }
    let previous_group = active_tab_id
        .filter(|id| !id.is_empty())
        .and_then(|id| find_group_of_tab(previous, id))?;
    if let Some(survivor) = next.iter().find(|group| group.id == previous_group.id) {
        return Some(survivor.active_tab_id.clone());
    }
    let index = previous
        .iter()
        .position(|group| group.id == previous_group.id)?;
    if let Some(after) = previous[index + 1..]
        .iter()
        .find(|group| next.iter().any(|candidate| candidate.id == group.id))
    {
        return Some(after.active_tab_id.clone());
    }
    next.last().map(|group| group.active_tab_id.clone())
}

/// Makes every tab sit in exactly one group: panes of tabs that are gone are
/// dropped, and tabs no group holds get a group of their own at the end.
pub fn reconcile_groups(
    groups: &[TabGroup],
    tab_ids: &[String],
    create_group_id: &mut dyn FnMut() -> String,
) -> Vec<TabGroup> {
    let known: HashSet<&str> = tab_ids.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    let mut stale = Vec::new();
    for group in groups {
        for id in get_pane_ids(&group.root) {
            if !known.contains(id.as_str()) || !seen.insert(id.clone()) {
                stale.push(id);
            }
        }
    }
    let mut result = if stale.is_empty() {
        groups.to_vec()
    } else {
        remove_tabs_from_groups(groups, &stale)
    };
    let placed: HashSet<String> = result
        .iter()
        .flat_map(|group| get_pane_ids(&group.root))
        .collect();
    let missing: Vec<&String> = tab_ids.iter().filter(|id| !placed.contains(*id)).collect();
    if !missing.is_empty() {
        result.extend(missing.into_iter().map(|tab_id| TabGroup {
            id: create_group_id(),
            root: pane_leaf(tab_id.clone()),
            active_tab_id: tab_id.clone(),
            zoomed_tab_id: None,
        }));
    }
    result
}

/// Makes `tab_id` its group's active pane; focusing a pane hidden by a zoom
/// ends the zoom.
pub fn focus_pane_in_groups(groups: &[TabGroup], tab_id: &str) -> Vec<TabGroup> {
    let Some(index) = groups
        .iter()
        .position(|group| get_pane_ids(&group.root).iter().any(|id| id == tab_id))
    else {
        return groups.to_vec();
    };
    let group = &groups[index];
    let zoomed_tab_id = group
        .zoomed_tab_id
        .as_ref()
        .filter(|id| *id == tab_id)
        .cloned();
    if group.active_tab_id == tab_id && group.zoomed_tab_id == zoomed_tab_id {
        return groups.to_vec();
    }
    let mut result = groups.to_vec();
    result[index] = TabGroup {
        id: group.id.clone(),
        root: group.root.clone(),
        active_tab_id: tab_id.to_owned(),
        zoomed_tab_id,
    };
    result
}

/// Moves a pane next to another one, out of its own group if need be. A group
/// the move leaves empty goes.
pub fn place_pane(
    groups: &[TabGroup],
    tab_id: &str,
    target_tab_id: &str,
    direction: SplitDirection,
) -> Vec<TabGroup> {
    if tab_id == target_tab_id || find_group_of_tab(groups, target_tab_id).is_none() {
        return groups.to_vec();
    }
    remove_tabs_from_groups(groups, &[tab_id.to_owned()])
        .into_iter()
        .map(|group| {
            if get_pane_ids(&group.root)
                .iter()
                .any(|id| id == target_tab_id)
            {
                TabGroup {
                    id: group.id,
                    root: insert_next_to(&group.root, target_tab_id, &pane_leaf(tab_id), direction),
                    active_tab_id: tab_id.to_owned(),
                    zoomed_tab_id: None,
                }
            } else {
                group
            }
        })
        .collect()
}

/// Moves every pane of the group holding `tab_id` next to `target_tab_id`,
/// splits and all.
pub fn merge_group(
    groups: &[TabGroup],
    tab_id: &str,
    target_tab_id: &str,
    direction: SplitDirection,
) -> Vec<TabGroup> {
    let Some(source_index) = groups
        .iter()
        .position(|group| get_pane_ids(&group.root).iter().any(|id| id == tab_id))
    else {
        return groups.to_vec();
    };
    let Some(target_index) = groups.iter().position(|group| {
        get_pane_ids(&group.root)
            .iter()
            .any(|id| id == target_tab_id)
    }) else {
        return groups.to_vec();
    };
    if source_index == target_index {
        return groups.to_vec();
    }
    let source = &groups[source_index];
    let target = &groups[target_index];
    let mut result = Vec::with_capacity(groups.len() - 1);
    for (index, group) in groups.iter().enumerate() {
        if index == source_index {
            continue;
        }
        if index == target_index {
            result.push(TabGroup {
                id: target.id.clone(),
                root: insert_next_to(&target.root, target_tab_id, &source.root, direction),
                active_tab_id: source.active_tab_id.clone(),
                zoomed_tab_id: None,
            });
        } else {
            result.push(group.clone());
        }
    }
    result
}

/// Takes a pane out of its group into a group of its own, right after it or at
/// `index`.
pub fn break_out_pane(
    groups: &[TabGroup],
    tab_id: &str,
    new_group_id: &str,
    index: Option<i64>,
) -> Vec<TabGroup> {
    let Some(source) = find_group_of_tab(groups, tab_id) else {
        return groups.to_vec();
    };
    if get_pane_ids(&source.root).len() < 2 {
        return groups.to_vec();
    }
    let source_id = source.id.clone();
    let mut result = remove_tabs_from_groups(groups, &[tab_id.to_owned()]);
    let at = index.unwrap_or_else(|| {
        result
            .iter()
            .position(|group| group.id == source_id)
            .map_or(0, |position| position as i64 + 1)
    });
    let clamped = at.clamp(0, result.len() as i64) as usize;
    result.insert(
        clamped,
        TabGroup {
            id: new_group_id.to_owned(),
            root: pane_leaf(tab_id),
            active_tab_id: tab_id.to_owned(),
            zoomed_tab_id: None,
        },
    );
    result
}

/// Gives every pane of the group holding `tab_id` a group of its own, in their
/// order.
pub fn ungroup_panes(
    groups: &[TabGroup],
    tab_id: &str,
    create_group_id: &mut dyn FnMut() -> String,
) -> Vec<TabGroup> {
    let Some(index) = groups
        .iter()
        .position(|group| get_pane_ids(&group.root).iter().any(|id| id == tab_id))
    else {
        return groups.to_vec();
    };
    let source = &groups[index];
    if matches!(source.root, PaneNode::Pane { .. }) {
        return groups.to_vec();
    }
    let singles: Vec<TabGroup> = get_pane_ids(&source.root)
        .into_iter()
        .map(|pane_id| TabGroup {
            id: if pane_id == source.active_tab_id {
                source.id.clone()
            } else {
                create_group_id()
            },
            root: pane_leaf(pane_id.clone()),
            active_tab_id: pane_id,
            zoomed_tab_id: None,
        })
        .collect();
    let mut result = Vec::with_capacity(groups.len() + singles.len() - 1);
    result.extend_from_slice(&groups[..index]);
    result.extend(singles);
    result.extend_from_slice(&groups[index + 1..]);
    result
}

/// Zooms the pane, or ends the zoom when it is the zoomed one; a lone pane has
/// nothing to zoom over.
pub fn toggle_zoom(groups: &[TabGroup], tab_id: &str) -> Vec<TabGroup> {
    let Some(index) = groups
        .iter()
        .position(|group| get_pane_ids(&group.root).iter().any(|id| id == tab_id))
    else {
        return groups.to_vec();
    };
    let group = &groups[index];
    if matches!(group.root, PaneNode::Pane { .. }) {
        return groups.to_vec();
    }
    let mut result = groups.to_vec();
    result[index] = TabGroup {
        id: group.id.clone(),
        root: group.root.clone(),
        active_tab_id: tab_id.to_owned(),
        zoomed_tab_id: if group.zoomed_tab_id.as_deref() == Some(tab_id) {
            None
        } else {
            Some(tab_id.to_owned())
        },
    };
    result
}

/// Moves a group from one position in the tab bar to another.
pub fn move_group(groups: &[TabGroup], from_index: i64, to_index: i64) -> Vec<TabGroup> {
    if from_index < 0
        || from_index >= groups.len() as i64
        || to_index < 0
        || to_index >= groups.len() as i64
        || from_index == to_index
    {
        return groups.to_vec();
    }
    let mut result = groups.to_vec();
    let moved = result.remove(from_index as usize);
    result.insert(to_index as usize, moved);
    result
}

/// The panes in sight: those of the active tab's group, or only its zoomed
/// one.
pub fn get_visible_pane_ids(groups: &[TabGroup], active_tab_id: Option<&str>) -> Vec<String> {
    let Some(group) = active_tab_id
        .filter(|id| !id.is_empty())
        .and_then(|id| find_group_of_tab(groups, id))
    else {
        return Vec::new();
    };
    match group
        .zoomed_tab_id
        .as_ref()
        .filter(|zoomed| !zoomed.is_empty())
    {
        Some(zoomed) => vec![zoomed.clone()],
        None => get_pane_ids(&group.root),
    }
}
