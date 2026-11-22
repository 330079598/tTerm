//! Nested split cards in the shared pane-tree model; tab ids stay opaque and stable.

use tterm_core::keymap::actions::KeymapActionId;
#[cfg(test)]
use tterm_core::workspace::pane_layout::get_split_at;
use tterm_core::workspace::pane_layout::{PaneBox, layout_pane_tree};
use tterm_core::workspace::pane_navigation::{
    PaneDirection, PaneRect, find_pane_in_direction, get_sibling_pane_id,
};
use tterm_core::workspace::tab_groups::{
    PaneNode, SplitDirection, TabGroup, find_group_of_tab, focus_pane_in_groups,
    get_fallback_tab_id, get_pane_ids, get_visible_pane_ids, pane_leaf, place_pane,
    remove_tabs_from_groups, resize_split, toggle_zoom, ungroup_panes,
};

pub(super) const FOCUS_ACTIONS: [KeymapActionId; 6] = [
    KeymapActionId::WorkspaceFocusNextPane,
    KeymapActionId::WorkspaceFocusPrevPane,
    KeymapActionId::WorkspaceFocusPaneLeft,
    KeymapActionId::WorkspaceFocusPaneRight,
    KeymapActionId::WorkspaceFocusPaneUp,
    KeymapActionId::WorkspaceFocusPaneDown,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    pub(super) fn direction(self) -> SplitDirection {
        match self {
            Edge::Left => SplitDirection::Left,
            Edge::Right => SplitDirection::Right,
            Edge::Top => SplitDirection::Above,
            Edge::Bottom => SplitDirection::Below,
        }
    }
}

/// One card per tab group; all panes refer to their owning tab by stable id.
#[derive(Default)]
pub(super) struct SplitState {
    groups: Vec<TabGroup>,
    last_group: u64,
}

impl SplitState {
    pub(super) fn new(first: usize) -> Self {
        let mut state = Self::default();
        state.open(first);
        state
    }

    /// Gives a tab that no card holds a card of its own.
    pub(super) fn open(&mut self, tab: usize) {
        if self.group_for(tab).is_some() {
            return;
        }
        let id = self.next_group_id();
        self.groups.push(TabGroup {
            id,
            root: pane_leaf(tab.to_string()),
            active_tab_id: tab.to_string(),
            zoomed_tab_id: None,
        });
    }

    pub(super) fn group_for(&self, tab: usize) -> Option<&TabGroup> {
        find_group_of_tab(&self.groups, &tab.to_string())
    }

    /// The full tree of the card holding `tab`, including panes hidden by a zoom.
    pub(super) fn root(&self, tab: usize) -> Option<&PaneNode> {
        self.group_for(tab).map(|group| &group.root)
    }

    pub(super) fn zoomed(&self, tab: usize) -> Option<usize> {
        self.group_for(tab)
            .and_then(|group| group.zoomed_tab_id.as_deref())
            .and_then(parse_tab_id)
    }

    pub(super) fn visible_ids(&self, active: usize) -> Vec<usize> {
        let active_key = active.to_string();
        let visible: Vec<usize> = get_visible_pane_ids(&self.groups, Some(&active_key))
            .iter()
            .filter_map(|id| parse_tab_id(id))
            .collect();
        if visible.is_empty() {
            vec![active]
        } else {
            visible
        }
    }

    pub(super) fn is_split(&self, tab: usize) -> bool {
        self.group_for(tab)
            .is_some_and(|group| get_pane_ids(&group.root).len() > 1)
    }

    pub(super) fn can_split(&self, source: usize, target: usize) -> bool {
        source != target && self.group_for(target).is_some()
    }

    /// Moves the existing view beside `target`; a card left without panes goes.
    pub(super) fn split(&mut self, source: usize, target: usize, edge: Edge) -> bool {
        let next = place_pane(
            &self.groups,
            &source.to_string(),
            &target.to_string(),
            edge.direction(),
        );
        if next == self.groups {
            return false;
        }
        self.groups = next;
        true
    }

    /// Gives every pane of the card its own card, keeping the sessions running.
    pub(super) fn ungroup(&mut self, tab: usize) -> bool {
        let mut next_id = self.last_group;
        let next = ungroup_panes(&self.groups, &tab.to_string(), &mut || {
            next_id = next_id.saturating_add(1);
            format!("group-{next_id}")
        });
        self.last_group = next_id;
        if next == self.groups {
            return false;
        }
        self.groups = next;
        true
    }

    pub(super) fn toggle_zoom(&mut self, tab: usize) -> bool {
        let next = toggle_zoom(&self.groups, &tab.to_string());
        if next == self.groups {
            return false;
        }
        self.groups = next;
        true
    }

    /// Focuses a pane, ending a zoom that hid it.
    pub(super) fn focus(&mut self, tab: usize) -> bool {
        let next = focus_pane_in_groups(&self.groups, &tab.to_string());
        if next == self.groups {
            return false;
        }
        self.groups = next;
        true
    }

    /// Sets the sizes of the split found by following child indexes from the root.
    pub(super) fn resize(&mut self, tab: usize, path: &[usize], sizes: &[f64]) -> bool {
        let tab_key = tab.to_string();
        let Some(group) = self
            .groups
            .iter_mut()
            .find(|group| get_pane_ids(&group.root).iter().any(|id| id == &tab_key))
        else {
            return false;
        };
        let path = path.iter().map(|index| *index as i64).collect::<Vec<_>>();
        let next = resize_split(&group.root, &path, sizes);
        if next == group.root {
            return false;
        }
        group.root = next;
        true
    }

    #[cfg(test)]
    pub(super) fn split_sizes(&self, tab: usize, path: &[usize]) -> Option<Vec<f64>> {
        let group = self.group_for(tab)?;
        match get_split_at(&group.root, path)? {
            PaneNode::Split { sizes, .. } => Some(sizes.clone()),
            PaneNode::Pane { .. } => None,
        }
    }

    /// Removes a pane and returns the tab to select after an active close.
    pub(super) fn close(&mut self, tab: usize) -> Option<usize> {
        let previous = self.groups.clone();
        let key = tab.to_string();
        find_group_of_tab(&previous, &key)?;
        self.groups = remove_tabs_from_groups(&previous, std::slice::from_ref(&key));
        get_fallback_tab_id(&previous, &self.groups, Some(&key)).and_then(|id| parse_tab_id(&id))
    }

    /// The pane a focus shortcut moves to, or `None` at an edge or while zoomed.
    pub(super) fn focus_target(&self, active: usize, action: KeymapActionId) -> Option<usize> {
        if !FOCUS_ACTIONS.contains(&action) {
            return None;
        }
        let group = self.group_for(active)?;
        if group.zoomed_tab_id.is_some() {
            return None;
        }
        let active_key = active.to_string();
        let rects = pane_rects(&group.root);
        let recent = get_pane_ids(&group.root);
        use KeymapActionId::*;
        let target = match action {
            WorkspaceFocusNextPane => get_sibling_pane_id(&rects, Some(&active_key), 1),
            WorkspaceFocusPrevPane => get_sibling_pane_id(&rects, Some(&active_key), -1),
            WorkspaceFocusPaneLeft => {
                find_pane_in_direction(&rects, Some(&active_key), PaneDirection::Left, &recent)
            }
            WorkspaceFocusPaneRight => {
                find_pane_in_direction(&rects, Some(&active_key), PaneDirection::Right, &recent)
            }
            WorkspaceFocusPaneUp => {
                find_pane_in_direction(&rects, Some(&active_key), PaneDirection::Up, &recent)
            }
            WorkspaceFocusPaneDown => {
                find_pane_in_direction(&rects, Some(&active_key), PaneDirection::Down, &recent)
            }
            _ => None,
        };
        target.and_then(|id| parse_tab_id(&id))
    }

    fn next_group_id(&mut self) -> String {
        self.last_group = self.last_group.saturating_add(1);
        format!("group-{}", self.last_group)
    }
}

fn parse_tab_id(id: &str) -> Option<usize> {
    id.parse().ok()
}

fn pane_rects(root: &PaneNode) -> Vec<PaneRect> {
    let layout = layout_pane_tree(
        root,
        PaneBox {
            left: 0.,
            top: 0.,
            width: 1000.,
            height: 1000.,
        },
        0.,
    );
    layout
        .panes
        .into_iter()
        .map(|(id, area)| PaneRect {
            id,
            left: area.left,
            top: area.top,
            right: area.left + area.width,
            bottom: area.top + area.height,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tterm_core::workspace::tab_groups::SplitOrientation;

    fn split_node(orientation: SplitOrientation, ids: &[usize], sizes: &[f64]) -> PaneNode {
        PaneNode::Split {
            orientation,
            children: ids
                .iter()
                .map(|id| PaneNode::Pane {
                    tab_id: id.to_string(),
                })
                .collect(),
            sizes: sizes.to_vec(),
        }
    }

    #[test]
    fn new_tabs_open_independent_cards_and_splitting_moves_the_view() {
        let mut state = SplitState::new(1);
        state.open(2);
        assert_eq!(state.visible_ids(1), vec![1]);
        assert_eq!(state.visible_ids(2), vec![2]);
        state.open(2);
        assert_eq!(state.groups.len(), 2);

        assert!(state.split(2, 1, Edge::Right));
        assert_eq!(state.visible_ids(1), vec![1, 2]);
        assert_eq!(state.visible_ids(2), vec![1, 2]);
        assert!(state.is_split(1));
        assert!(state.is_split(2));
        assert_eq!(
            state.root(1),
            Some(&split_node(
                SplitOrientation::Horizontal,
                &[1, 2],
                &[0.5, 0.5]
            ))
        );
    }

    #[test]
    fn splitting_a_child_nests_without_flattening_and_close_keeps_root_sizes() {
        let mut state = SplitState::new(1);
        state.open(2);
        state.open(3);
        assert!(state.split(2, 1, Edge::Right));
        assert!(state.split(3, 2, Edge::Bottom));
        assert!(state.resize(1, &[], &[0.3, 0.7]));

        let Some(PaneNode::Split {
            orientation,
            children,
            sizes,
        }) = state.root(1)
        else {
            panic!("expected a horizontal root");
        };
        assert_eq!(*orientation, SplitOrientation::Horizontal);
        assert_eq!(sizes, &[0.3, 0.7]);
        assert_eq!(
            children[1],
            split_node(SplitOrientation::Vertical, &[2, 3], &[0.5, 0.5])
        );

        assert_eq!(state.close(3), Some(2));
        assert_eq!(state.split_sizes(2, &[]), Some(vec![0.3, 0.7]));
        assert_eq!(
            state.root(1),
            Some(&split_node(
                SplitOrientation::Horizontal,
                &[1, 2],
                &[0.3, 0.7]
            ))
        );
    }

    #[test]
    fn zoom_hides_other_panes_and_focusing_them_reveals_the_card() {
        let mut state = SplitState::new(1);
        state.open(2);
        state.split(2, 1, Edge::Right);

        assert!(state.toggle_zoom(1));
        assert_eq!(state.zoomed(1), Some(1));
        assert_eq!(state.visible_ids(1), vec![1]);
        assert_eq!(
            state.focus_target(1, KeymapActionId::WorkspaceFocusPaneRight),
            None
        );

        assert!(state.focus(2));
        assert_eq!(state.zoomed(1), None);
        assert_eq!(state.visible_ids(2), vec![1, 2]);
        assert!(!state.toggle_zoom(3));
    }

    #[test]
    fn ungroup_gives_each_pane_its_own_card_and_close_returns_the_survivor() {
        let mut state = SplitState::new(1);
        state.open(2);
        state.split(2, 1, Edge::Top);

        assert_eq!(state.close(2), Some(1));
        assert_eq!(state.visible_ids(1), vec![1]);
        assert!(!state.is_split(1));
        assert_eq!(state.close(1), None);

        let mut state = SplitState::new(1);
        state.open(2);
        state.split(2, 1, Edge::Left);
        assert!(state.ungroup(2));
        assert_eq!(state.visible_ids(1), vec![1]);
        assert_eq!(state.visible_ids(2), vec![2]);
        assert!(!state.is_split(2));
    }

    #[test]
    fn directional_focus_follows_the_nested_geometry_and_cycles_in_reading_order() {
        let mut state = SplitState::new(1);
        state.open(2);
        state.open(3);
        state.split(2, 1, Edge::Right);
        state.split(3, 2, Edge::Bottom);

        use KeymapActionId::*;
        assert_eq!(state.focus_target(1, WorkspaceFocusPaneRight), Some(2));
        // 3 is below 2, not below 1, so 1 has no downward neighbour.
        assert_eq!(state.focus_target(1, WorkspaceFocusPaneDown), None);
        assert_eq!(state.focus_target(2, WorkspaceFocusPaneDown), Some(3));
        assert_eq!(state.focus_target(3, WorkspaceFocusPaneUp), Some(2));
        assert_eq!(state.focus_target(1, WorkspaceFocusNextPane), Some(2));
        assert_eq!(state.focus_target(3, WorkspaceFocusNextPane), Some(1));
        assert_eq!(state.focus_target(3, WorkspaceFocusPrevPane), Some(2));
    }

    #[test]
    fn resize_rejects_stale_paths_and_mismatched_sizes() {
        let mut state = SplitState::new(1);
        state.open(2);
        state.split(2, 1, Edge::Right);
        assert!(!state.resize(3, &[], &[0.25, 0.75]));
        assert!(!state.resize(1, &[], &[0.25]));
        assert!(!state.resize(1, &[4], &[0.25, 0.75]));
        assert_eq!(state.split_sizes(1, &[]), Some(vec![0.5, 0.5]));
        assert!(state.resize(1, &[], &[0.25, 0.75]));
        assert_eq!(state.split_sizes(1, &[]), Some(vec![0.25, 0.75]));
        let ids = get_pane_ids(state.root(1).unwrap());
        assert_eq!(ids, vec!["1".to_string(), "2".to_string()]);
    }
}
