//! Split membership references stable tab IDs; the workspace keeps ownership of each session.

use tterm_core::keymap::actions::KeymapActionId;
use tterm_core::workspace::tab_groups::SplitOrientation;

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Pair {
    pub(super) first: usize,
    pub(super) second: usize,
    pub(super) axis: SplitOrientation,
    /// The first pane's share of the space remaining after the divider.
    pub(super) ratio: f32,
}

impl Pair {
    pub(super) fn focus_target(self, active: usize, action: KeymapActionId) -> Option<usize> {
        use KeymapActionId::*;
        use SplitOrientation::{Horizontal, Vertical};
        match (self.axis, action) {
            (_, WorkspaceFocusNextPane | WorkspaceFocusPrevPane) if active == self.first => {
                Some(self.second)
            }
            (_, WorkspaceFocusNextPane | WorkspaceFocusPrevPane) if active == self.second => {
                Some(self.first)
            }
            (Horizontal, WorkspaceFocusPaneRight) | (Vertical, WorkspaceFocusPaneDown)
                if active == self.first =>
            {
                Some(self.second)
            }
            (Horizontal, WorkspaceFocusPaneLeft) | (Vertical, WorkspaceFocusPaneUp)
                if active == self.second =>
            {
                Some(self.first)
            }
            _ => None,
        }
    }

    fn contains(self, id: usize) -> bool {
        self.first == id || self.second == id
    }
}

#[derive(Default)]
pub(super) struct SplitState {
    pairs: Vec<Pair>,
}

impl SplitState {
    pub(super) fn pair_for(&self, id: usize) -> Option<Pair> {
        self.pairs.iter().copied().find(|pair| pair.contains(id))
    }

    pub(super) fn can_split(&self, source: usize, target: usize) -> bool {
        source != target && self.pair_for(source).is_none() && self.pair_for(target).is_none()
    }

    pub(super) fn split(&mut self, source: usize, target: usize, edge: Edge) -> bool {
        if !self.can_split(source, target) {
            return false;
        }
        let (first, second, axis) = match edge {
            Edge::Left => (source, target, SplitOrientation::Horizontal),
            Edge::Right => (target, source, SplitOrientation::Horizontal),
            Edge::Top => (source, target, SplitOrientation::Vertical),
            Edge::Bottom => (target, source, SplitOrientation::Vertical),
        };
        self.pairs.push(Pair {
            first,
            second,
            axis,
            ratio: 0.5,
        });
        true
    }

    pub(super) fn resize(&mut self, id: usize, ratio: f32) -> bool {
        if !ratio.is_finite() || ratio <= 0.0 || ratio >= 1.0 {
            return false;
        }
        let Some(pair) = self.pairs.iter_mut().find(|pair| pair.contains(id)) else {
            return false;
        };
        if pair.ratio == ratio {
            return false;
        }
        pair.ratio = ratio;
        true
    }

    /// Dissolves a closed tab's pair and returns the surviving session to select.
    pub(super) fn remove(&mut self, id: usize) -> Option<usize> {
        let index = self.pairs.iter().position(|pair| pair.contains(id))?;
        let pair = self.pairs.swap_remove(index);
        Some(if id == pair.first {
            pair.second
        } else {
            pair.first
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directional_focus_stops_at_edges_but_reading_order_wraps() {
        use KeymapActionId::*;
        for (axis, forward, backward, cross) in [
            (
                SplitOrientation::Horizontal,
                WorkspaceFocusPaneRight,
                WorkspaceFocusPaneLeft,
                WorkspaceFocusPaneUp,
            ),
            (
                SplitOrientation::Vertical,
                WorkspaceFocusPaneDown,
                WorkspaceFocusPaneUp,
                WorkspaceFocusPaneRight,
            ),
        ] {
            let pair = Pair {
                axis,
                first: 30,
                second: 10,
                ratio: 0.4,
            };
            assert_eq!(pair.focus_target(30, forward), Some(10));
            assert_eq!(pair.focus_target(10, backward), Some(30));
            assert_eq!(pair.focus_target(30, backward), None);
            assert_eq!(pair.focus_target(10, forward), None);
            assert_eq!(pair.focus_target(10, cross), None);
            for cycle in [WorkspaceFocusNextPane, WorkspaceFocusPrevPane] {
                assert_eq!(pair.focus_target(30, cycle), Some(10));
                assert_eq!(pair.focus_target(10, cycle), Some(30));
                assert_eq!(pair.focus_target(42, cycle), None);
            }
        }
    }

    #[test]
    fn every_edge_places_the_source_on_the_requested_side() {
        for (edge, first, second, axis) in [
            (Edge::Left, 10, 20, SplitOrientation::Horizontal),
            (Edge::Right, 20, 10, SplitOrientation::Horizontal),
            (Edge::Top, 10, 20, SplitOrientation::Vertical),
            (Edge::Bottom, 20, 10, SplitOrientation::Vertical),
        ] {
            let mut state = SplitState::default();
            assert!(state.split(10, 20, edge));
            assert_eq!(
                state.pair_for(first),
                Some(Pair {
                    first,
                    second,
                    axis,
                    ratio: 0.5,
                })
            );
            assert_eq!(state.pair_for(first), state.pair_for(second));
        }
    }

    #[test]
    fn invalid_drop_never_changes_existing_membership() {
        let mut state = SplitState::default();
        assert!(!state.split(1, 1, Edge::Left));
        assert!(state.split(1, 2, Edge::Left));
        let original = state.pair_for(1);
        assert!(!state.split(1, 3, Edge::Right));
        assert!(!state.split(3, 2, Edge::Top));
        assert!(!state.split(2, 1, Edge::Bottom));
        assert_eq!(state.pair_for(2), original);
        assert!(state.pair_for(3).is_none());
    }

    #[test]
    fn closing_either_side_only_dissolves_that_pair() {
        let mut state = SplitState::default();
        assert!(state.split(1, 2, Edge::Left));
        assert!(state.split(3, 4, Edge::Top));
        let other = state.pair_for(2);
        assert_eq!(state.remove(4), Some(3));
        assert_eq!(state.remove(4), None);
        assert!(state.pair_for(3).is_none());
        assert_eq!(state.pair_for(1), other);
        assert_eq!(state.remove(1), Some(2));
        assert!(state.split(2, 3, Edge::Right));
    }

    #[test]
    fn resize_updates_only_the_named_pair_from_either_member() {
        let mut state = SplitState::default();
        state.split(1, 2, Edge::Right);
        state.split(3, 4, Edge::Bottom);
        let other = state.pair_for(4);
        assert!(state.resize(1, 0.25));
        assert_eq!(state.pair_for(2).unwrap().ratio, 0.25);
        assert!(state.resize(2, 0.75));
        assert_eq!(state.pair_for(1).unwrap().ratio, 0.75);
        assert_eq!(state.pair_for(3), other);
    }

    #[test]
    fn invalid_or_stale_resize_leaves_both_panes_intact() {
        let mut state = SplitState::default();
        state.split(1, 2, Edge::Right);
        for ratio in [
            0.,
            1.,
            -0.1,
            1.1,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            0.5,
        ] {
            assert!(!state.resize(1, ratio));
        }
        assert!(!state.resize(3, 0.3));
        assert_eq!(state.pair_for(2).unwrap().ratio, 0.5);
        state.remove(1);
        assert!(!state.resize(1, 0.3));
        assert!(!state.resize(2, 0.3));
    }
}
