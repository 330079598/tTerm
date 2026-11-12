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

fn compare_reading_order(first: &PaneRect, second: &PaneRect) -> f64 {
    let vertical_distance = first.top - second.top;
    if vertical_distance.abs() > EDGE_TOLERANCE {
        vertical_distance
    } else {
        first.left - second.left
    }
}

// V8's Array.prototype.sort uses TimSort. For fewer than 64 elements it
// counts the first run and then binary-insertion-sorts the rest; reproducing
// that path keeps the order for comparisons that are not transitive.
fn count_and_make_run(panes: &mut [PaneRect], low_argument: usize, high: usize) -> usize {
    let low = low_argument + 1;
    if low == high {
        return 1;
    }

    let mut run_length = 2;
    let mut order = compare_reading_order(&panes[low], &panes[low - 1]);
    let descending = order < 0.0;
    let mut previous = low;
    for index in low + 1..high {
        order = compare_reading_order(&panes[index], &panes[previous]);
        if descending {
            if order >= 0.0 {
                break;
            }
        } else if order < 0.0 {
            break;
        }
        previous = index;
        run_length += 1;
    }

    if descending {
        panes[low_argument..low_argument + run_length].reverse();
    }
    run_length
}

fn binary_insertion_sort(panes: &mut [PaneRect], low: usize, start_argument: usize, high: usize) {
    let start = if low == start_argument {
        start_argument + 1
    } else {
        start_argument
    };
    for start in start..high {
        let pivot = panes[start].clone();
        let mut left = low;
        let mut right = start;
        while left < right {
            let middle = left + (right - left) / 2;
            let order = compare_reading_order(&pivot, &panes[middle]);
            if order < 0.0 {
                right = middle;
            } else {
                left = middle + 1;
            }
        }
        for position in (left..start).rev() {
            panes.swap(position, position + 1);
        }
        panes[left] = pivot;
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
    if ordered.len() >= 2 {
        // V8 switches to the merging path at 64 elements. This port keeps the
        // short-array algorithm for every length so it cannot panic on a
        // comparison that is not a total order; its order above 63 elements
        // may differ from V8's.
        let length = ordered.len();
        let run_length = count_and_make_run(&mut ordered, 0, length);
        if run_length < length {
            binary_insertion_sort(&mut ordered, 0, run_length, length);
        }
    }
    ordered
}

/// Returns the pane `offset` steps away from the active one in reading order,
/// wrapping around both ends.
pub fn get_sibling_pane_id(
    panes: &[PaneRect],
    active_id: Option<&str>,
    offset: i64,
) -> Option<String> {
    let active_id = active_id?;
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
    let active_id = active_id?;
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
