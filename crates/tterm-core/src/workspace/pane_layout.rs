//! Pane and divider geometry, from `src/lib/paneLayout.ts`.

use indexmap::IndexMap;

use super::tab_groups::{PaneNode, SplitOrientation};

/// A rectangle in client pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneBox {
    /// Distance from the left edge of the layout.
    pub left: f64,
    /// Distance from the top edge of the layout.
    pub top: f64,
    /// Width in pixels.
    pub width: f64,
    /// Height in pixels.
    pub height: f64,
}

/// The divider between a split's children `index` and `index + 1`.
#[derive(Debug, Clone, PartialEq)]
pub struct SashBox {
    /// Distance from the left edge of the layout.
    pub left: f64,
    /// Distance from the top edge of the layout.
    pub top: f64,
    /// Width in pixels.
    pub width: f64,
    /// Height in pixels.
    pub height: f64,
    /// The split's child index at each level from the root.
    pub path: Vec<usize>,
    /// The index of the child to the left or above the divider.
    pub index: usize,
    /// The axis the divider moves along.
    pub orientation: SplitOrientation,
    /// The split's length along its orientation, less the gaps.
    pub room: f64,
}

/// Pane rectangles and dividers laid out for one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneLayout {
    /// Pane boxes by tab id, in tree order.
    pub panes: IndexMap<String, PaneBox>,
    /// The dividers between the panes.
    pub sashes: Vec<SashBox>,
}

/// The least room one pane needs on each axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneMinimum {
    /// The least width a pane needs.
    pub width: f64,
    /// The least height a pane needs.
    pub height: f64,
}

fn along(orientation: SplitOrientation, area: PaneBox) -> f64 {
    match orientation {
        SplitOrientation::Horizontal => area.width,
        SplitOrientation::Vertical => area.height,
    }
}

/// Rounds half toward positive infinity, as `Math.round` does.
pub(crate) fn js_round(value: f64) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    if value.abs() >= 4_503_599_627_370_496.0 {
        return value;
    }
    if value > 0.0 {
        let floor = value.floor();
        if value - floor >= 0.5 {
            floor + 1.0
        } else {
            floor
        }
    } else {
        let ceiling = value.ceil();
        if value - ceiling < -0.5 {
            ceiling - 1.0
        } else {
            ceiling
        }
    }
}

fn js_max_zero(value: f64) -> f64 {
    if value.is_nan() {
        f64::NAN
    } else {
        value.max(0.0)
    }
}

fn js_max(first: f64, second: f64) -> f64 {
    if first.is_nan() || second.is_nan() {
        f64::NAN
    } else {
        first.max(second)
    }
}

fn js_min(first: f64, second: f64) -> f64 {
    if first.is_nan() || second.is_nan() {
        f64::NAN
    } else {
        first.min(second)
    }
}

/// Lays a tree out in `box`, leaving `gap` pixels between panes.
pub fn layout_pane_tree(root: &PaneNode, area: PaneBox, gap: f64) -> PaneLayout {
    let mut layout = PaneLayout {
        panes: IndexMap::new(),
        sashes: Vec::new(),
    };
    let mut path = Vec::new();
    visit(root, area, gap, &mut path, &mut layout);
    layout
}

fn visit(node: &PaneNode, area: PaneBox, gap: f64, path: &mut Vec<usize>, layout: &mut PaneLayout) {
    match node {
        PaneNode::Pane { tab_id } => {
            layout.panes.insert(tab_id.clone(), area);
        }
        PaneNode::Split {
            orientation,
            children,
            sizes,
        } => {
            let horizontal = *orientation == SplitOrientation::Horizontal;
            let start = if horizontal { area.left } else { area.top };
            let room = js_max_zero(along(*orientation, area) - gap * (children.len() as f64 - 1.0));
            let mut share = 0.0;
            let mut edge = start;
            for (index, child) in children.iter().enumerate() {
                share += sizes.get(index).copied().unwrap_or(f64::NAN);
                let is_last = index + 1 == children.len();
                let end = if is_last {
                    start + along(*orientation, area)
                } else {
                    js_round(start + room * share + gap * index as f64)
                };
                let child_area = if horizontal {
                    PaneBox {
                        left: edge,
                        top: area.top,
                        width: end - edge,
                        height: area.height,
                    }
                } else {
                    PaneBox {
                        left: area.left,
                        top: edge,
                        width: area.width,
                        height: end - edge,
                    }
                };
                path.push(index);
                visit(child, child_area, gap, path, layout);
                path.pop();
                if !is_last {
                    layout.sashes.push(if horizontal {
                        SashBox {
                            left: end,
                            top: area.top,
                            width: gap,
                            height: area.height,
                            path: path.clone(),
                            index,
                            orientation: *orientation,
                            room,
                        }
                    } else {
                        SashBox {
                            left: area.left,
                            top: end,
                            width: area.width,
                            height: gap,
                            path: path.clone(),
                            index,
                            orientation: *orientation,
                            room,
                        }
                    });
                }
                edge = end + gap;
            }
        }
    }
}

/// The least room a tree needs along `orientation` without a pane dropping
/// under `minimum`.
pub fn minimum_extent(
    node: &PaneNode,
    orientation: SplitOrientation,
    minimum: PaneMinimum,
    gap: f64,
) -> f64 {
    match node {
        PaneNode::Pane { .. } => match orientation {
            SplitOrientation::Horizontal => minimum.width,
            SplitOrientation::Vertical => minimum.height,
        },
        PaneNode::Split {
            orientation: split_orientation,
            children,
            ..
        } => {
            let extents: Vec<f64> = children
                .iter()
                .map(|child| minimum_extent(child, orientation, minimum, gap))
                .collect();
            if *split_orientation == orientation {
                extents.iter().sum::<f64>() + gap * (children.len() as f64 - 1.0)
            } else {
                // `Math.max` answers NaN when any argument is NaN, while
                // `f64::max` ignores NaN.
                extents
                    .into_iter()
                    .fold(f64::NEG_INFINITY, |widest, extent| {
                        if widest.is_nan() || extent.is_nan() {
                            f64::NAN
                        } else {
                            widest.max(extent)
                        }
                    })
            }
        }
    }
}

/// The split's sizes after the divider after child `index` moved `delta`
/// pixels. Only its two neighbours change, and neither shrinks under its
/// minimum unless it was already under it.
pub fn drag_sash(sizes: &[f64], index: i64, delta: f64, room: f64, minimums: &[f64]) -> Vec<f64> {
    if room <= 0.0
        || index < 0
        || index
            .checked_add(1)
            .is_none_or(|next| next >= sizes.len() as i64)
    {
        return sizes.to_vec();
    }
    let index = index as usize;
    let before = sizes[index] * room;
    let after = sizes[index + 1] * room;
    let lowest = -js_max(
        0.0,
        before - minimums.get(index).copied().unwrap_or(f64::NAN),
    );
    let highest = js_max(
        0.0,
        after - minimums.get(index + 1).copied().unwrap_or(f64::NAN),
    );
    let moved = js_min(js_max(delta, lowest), highest);
    let mut next = sizes.to_vec();
    next[index] = (before + moved) / room;
    next[index + 1] = (after - moved) / room;
    next
}

/// The split found by following `path` from the root.
pub fn get_split_at<'a>(root: &'a PaneNode, path: &[usize]) -> Option<&'a PaneNode> {
    let mut node = root;
    for index in path {
        node = match node {
            PaneNode::Split { children, .. } => children.get(*index)?,
            PaneNode::Pane { .. } => return None,
        };
    }
    match node {
        PaneNode::Split { .. } => Some(node),
        PaneNode::Pane { .. } => None,
    }
}
