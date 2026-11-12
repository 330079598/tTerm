//! The `closedTabs.ts` placement helpers needed by the reducer's reopen action.

use crate::workspace::pane_templates::{LayoutShape, fill_shape};
use crate::workspace::tab_groups::{
    PaneNode, SplitDirection, TabGroup, find_group_of_tab, insert_next_to, pane_leaf, remove_panes,
};

/// The surviving pane beside which a closed pane is reopened.
#[derive(Debug, Clone)]
pub struct PaneAnchor {
    /// The surviving tab id.
    pub tab_id: String,
    /// The side on which to insert the reopened pane.
    pub direction: SplitDirection,
}

/// The group or pane layout recorded when tabs closed.
#[derive(Debug, Clone)]
pub enum ClosedLayout {
    /// A complete group restored at its original position.
    Group {
        /// The group's split shape.
        shape: LayoutShape,
        /// The index of its previously focused pane.
        active_index: i64,
        /// Its position in the tab bar.
        index: i64,
    },
    /// A single pane restored into a surviving group.
    Pane {
        /// The pane's old id.
        closed_id: String,
        /// The group's root before the pane closed.
        previous_root: PaneNode,
        /// A surviving neighbor, when one existed.
        anchor: Option<PaneAnchor>,
    },
}

fn rename_pane(node: &PaneNode, from: &str, to: &str) -> PaneNode {
    match node {
        PaneNode::Pane { tab_id } if tab_id == from => pane_leaf(to),
        PaneNode::Pane { .. } => node.clone(),
        PaneNode::Split {
            orientation,
            children,
            sizes,
        } => PaneNode::Split {
            orientation: *orientation,
            children: children
                .iter()
                .map(|child| rename_pane(child, from, to))
                .collect(),
            sizes: sizes.clone(),
        },
    }
}

/// The pane activated by a nonempty reopen action.
pub fn reopened_active_tab_id(layout: &ClosedLayout, ids: &[String]) -> Option<String> {
    match layout {
        ClosedLayout::Group { active_index, .. } => usize::try_from(*active_index)
            .ok()
            .and_then(|index| ids.get(index))
            .or_else(|| ids.first())
            .cloned(),
        ClosedLayout::Pane { .. } => ids.first().cloned(),
    }
}

/// Restores a closed group or pane using the TypeScript placement rules.
pub fn place_reopened_tabs(
    groups: &[TabGroup],
    layout: &ClosedLayout,
    ids: &[String],
    create_group_id: &mut dyn FnMut() -> String,
) -> Vec<TabGroup> {
    let Some(tab_id) = ids.first() else {
        return groups.to_vec();
    };
    match layout {
        ClosedLayout::Group { shape, index, .. } => {
            let mut result = groups.to_vec();
            let group = TabGroup {
                id: create_group_id(),
                root: fill_shape(shape, ids),
                active_tab_id: reopened_active_tab_id(layout, ids).unwrap_or_default(),
                zoomed_tab_id: None,
            };
            result.insert((*index).clamp(0, result.len() as i64) as usize, group);
            result
        }
        ClosedLayout::Pane {
            closed_id,
            previous_root,
            anchor,
        } => {
            let Some(anchor) = anchor else {
                return groups.to_vec();
            };
            let Some(target) = find_group_of_tab(groups, &anchor.tab_id) else {
                return groups.to_vec();
            };
            let previous = rename_pane(previous_root, closed_id, tab_id);
            let stripped = remove_panes(&previous, std::slice::from_ref(tab_id));
            let same_tree =
                serde_json::to_string(&stripped).ok() == serde_json::to_string(&target.root).ok();
            let root = if same_tree {
                previous
            } else {
                insert_next_to(
                    &target.root,
                    &anchor.tab_id,
                    &pane_leaf(tab_id),
                    anchor.direction,
                )
            };
            groups
                .iter()
                .map(|group| {
                    if std::ptr::eq(group, target) {
                        TabGroup {
                            root: root.clone(),
                            active_tab_id: tab_id.clone(),
                            zoomed_tab_id: None,
                            ..group.clone()
                        }
                    } else {
                        group.clone()
                    }
                })
                .collect()
        }
    }
}
