//! Choosing the pane a directional shortcut moves to, from
//! `src/lib/paneNavigation.ts`.

use std::cmp::Ordering;

/// A direction a pane shortcut can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneDirection {
    /// The pane to the left.
    Left,
    /// The pane to the right.
    Right,
    /// The pane above.
    Up,
    /// The pane below.
    Down,
}

/// A pane's id and its client-pixel rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneRect {
    /// The pane's tab id.
    pub id: String,
    /// Distance from the left edge of the workspace.
    pub left: f64,
    /// Distance from the top edge of the workspace.
    pub top: f64,
    /// Distance of the right edge from the left edge of the workspace.
    pub right: f64,
    /// Distance of the bottom edge from the top edge of the workspace.
    pub bottom: f64,
}

const EDGE_TOLERANCE: f64 = 2.0;

fn is_visible(pane: &PaneRect) -> bool {
    pane.right > pane.left && pane.bottom > pane.top
}

fn compare_reading_order(first: &PaneRect, second: &PaneRect) -> Ordering {
    if (first.top - second.top).abs() > EDGE_TOLERANCE {
        first
            .top
            .partial_cmp(&second.top)
            .unwrap_or(Ordering::Equal)
    } else {
        first
            .left
            .partial_cmp(&second.left)
            .unwrap_or(Ordering::Equal)
    }
}

/// Orders visible panes as they read on screen: top to bottom, then left to
/// right. Panes with no size are left out.
pub fn sort_panes_by_reading_order(panes: &[PaneRect]) -> Vec<PaneRect> {
    let mut ordered: Vec<PaneRect> = panes
        .iter()
        .filter(|pane| is_visible(pane))
        .cloned()
        .collect();
    ordered.sort_by(compare_reading_order);
    ordered
}

/// Returns the pane `offset` steps away from the active one in reading order,
/// wrapping around both ends.
pub fn get_sibling_pane_id(
    panes: &[PaneRect],
    active_id: Option<&str>,
    offset: i64,
) -> Option<String> {
    let active_id = active_id.filter(|id| !id.is_empty())?;
    let ordered = sort_panes_by_reading_order(panes);
    let index = ordered.iter().position(|pane| pane.id == active_id)?;
    if ordered.len() < 2 {
        return None;
    }
    let next = (index as i128 + offset as i128 + ordered.len() as i128) % ordered.len() as i128;
    if next < 0 {
        return None;
    }
    ordered.get(next as usize).map(|pane| pane.id.clone())
}

fn distance_beyond(active: &PaneRect, pane: &PaneRect, direction: PaneDirection) -> Option<f64> {
    let distance = match direction {
        PaneDirection::Left => active.left - pane.right,
        PaneDirection::Right => pane.left - active.right,
        PaneDirection::Up => active.top - pane.bottom,
        PaneDirection::Down => pane.top - active.bottom,
    };
    if distance >= -EDGE_TOLERANCE {
        Some(distance)
    } else {
        None
    }
}

fn shared_edge(active: &PaneRect, pane: &PaneRect, direction: PaneDirection) -> f64 {
    match direction {
        PaneDirection::Left | PaneDirection::Right => {
            active.bottom.min(pane.bottom) - active.top.max(pane.top)
        }
        PaneDirection::Up | PaneDirection::Down => {
            active.right.min(pane.right) - active.left.max(pane.left)
        }
    }
}

fn recency(id: &str, recent_ids: &[String]) -> f64 {
    let index = recent_ids.iter().position(|recent| recent == id);
    match index {
        Some(index) => index as f64,
        None => f64::INFINITY,
    }
}

/// Returns the pane next to the active one in `direction`, without wrapping.
/// Of the nearest panes on that side, the one sharing most of the edge wins,
/// then the one listed first in `recent_ids`.
pub fn find_pane_in_direction(
    panes: &[PaneRect],
    active_id: Option<&str>,
    direction: PaneDirection,
    recent_ids: &[String],
) -> Option<String> {
    let visible: Vec<&PaneRect> = panes.iter().filter(|pane| is_visible(pane)).collect();
    let active_id = active_id.filter(|id| !id.is_empty())?;
    let active = visible.iter().find(|pane| pane.id == active_id)?;

    #[derive(Clone)]
    struct Candidate {
        id: String,
        distance: f64,
        shared: f64,
    }

    let mut candidates = Vec::new();
    for pane in &visible {
        if pane.id == active.id {
            continue;
        }
        let Some(distance) = distance_beyond(active, pane, direction) else {
            continue;
        };
        let shared = shared_edge(active, pane, direction);
        if shared <= 0.0 {
            continue;
        }
        candidates.push(Candidate {
            id: pane.id.clone(),
            distance,
            shared,
        });
    }
    if candidates.is_empty() {
        return None;
    }

    let nearest = candidates.iter().fold(f64::INFINITY, |nearest, candidate| {
        nearest.min(candidate.distance)
    });
    let mut nearest_candidates: Vec<Candidate> = candidates
        .into_iter()
        .filter(|candidate| candidate.distance - nearest <= EDGE_TOLERANCE)
        .collect();
    nearest_candidates.sort_by(|first, second| {
        second
            .shared
            .partial_cmp(&first.shared)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                recency(&first.id, recent_ids)
                    .partial_cmp(&recency(&second.id, recent_ids))
                    .unwrap_or(Ordering::Equal)
            })
    });
    nearest_candidates
        .first()
        .map(|candidate| candidate.id.clone())
}
