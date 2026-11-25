//! Split cards render the whole pane tree, and a tab drop moves the existing
//! terminal view beside the leaf under the pointer.

use std::{cell::Cell, rc::Rc};

use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, Context, Div, Hsla, Pixels, Point, Window, canvas, div, fill, outline, px,
};
use tterm_core::workspace::{
    pane_layout::{PaneMinimum, minimum_extent},
    tab_groups::{PaneNode, SplitOrientation, get_pane_ids},
};
use tterm_ui::tokens::gpui::hsla;

use super::{Prototype, drag::TabDrag, split_state::Edge};
mod sash;

#[cfg(test)]
mod tests;

fn edge_at(bounds: Bounds<Pixels>, point: Point<Pixels>) -> Option<Edge> {
    if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) || !bounds.contains(&point) {
        return None;
    }
    let horizontal = f32::from(point.x - bounds.left()) / f32::from(bounds.size.width) - 0.5;
    let vertical = f32::from(point.y - bounds.top()) / f32::from(bounds.size.height) - 0.5;
    if horizontal.abs() > vertical.abs() {
        Some(if horizontal < 0. {
            Edge::Left
        } else {
            Edge::Right
        })
    } else {
        Some(if vertical < 0. {
            Edge::Top
        } else {
            Edge::Bottom
        })
    }
}

fn preview_bounds(mut bounds: Bounds<Pixels>, edge: Edge) -> Bounds<Pixels> {
    match edge {
        Edge::Left | Edge::Right => {
            bounds.size.width *= 0.5;
            if edge == Edge::Right {
                bounds.origin.x += bounds.size.width;
            }
        }
        Edge::Top | Edge::Bottom => {
            bounds.size.height *= 0.5;
            if edge == Edge::Bottom {
                bounds.origin.y += bounds.size.height;
            }
        }
    }
    bounds.inset(
        px(6.)
            .min(bounds.size.width * 0.25)
            .min(bounds.size.height * 0.25),
    )
}

fn leaf_drop_surface(
    tab_id: usize,
    foreground: Hsla,
    background: Hsla,
    commit: impl Fn(&TabDrag, Edge, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    named_drop_surface(
        format!("split-drop-surface-{tab_id}"),
        format!("workspace-pane-drop-{tab_id}"),
        foreground,
        background,
        commit,
    )
}

fn named_drop_surface(
    id: String,
    selector: String,
    foreground: Hsla,
    background: Hsla,
    commit: impl Fn(&TabDrag, Edge, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let measured = Rc::new(Cell::new(None::<Bounds<Pixels>>));
    let target = measured.clone();
    div()
        .id(id)
        .debug_selector(move || selector)
        .absolute()
        .inset_0()
        .occlude()
        .child(
            canvas(
                move |bounds, _, _| measured.set(Some(bounds)),
                move |bounds, (), window, _| {
                    if let Some(edge) = edge_at(bounds, window.mouse_position()) {
                        let preview = preview_bounds(bounds, edge);
                        window.paint_quad(fill(preview, background));
                        window.paint_quad(outline(preview, foreground, Default::default()));
                    }
                },
            )
            .absolute()
            .inset_0()
            .size_full(),
        )
        .on_drop(move |drag: &TabDrag, window, cx| {
            if let Some(edge) = target
                .get()
                .and_then(|bounds| edge_at(bounds, window.mouse_position()))
            {
                commit(drag, edge, window, cx);
            }
        })
}

impl Prototype {
    pub(super) fn workspace_panes(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let active = self.tabs.active_id();
        let source = self.drag_layout.gesture.get().and_then(|(id, _)| {
            (cx.has_active_drag() && self.tabs.get(id).is_some()).then_some(id)
        });
        let root = div().size_full().relative().flex().min_w_0().min_h_0();
        let Some(tree) = self.splits.root(active).cloned() else {
            return root.child(self.render_pane(active, source, cx));
        };
        if let Some(zoomed) = self.splits.zoomed(active) {
            return root.child(self.render_pane(zoomed, source, cx));
        }
        root.child(self.render_node(&tree, Vec::new(), active, source, window, cx))
    }

    fn render_node(
        &self,
        node: &PaneNode,
        path: Vec<usize>,
        active: usize,
        source: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        match node {
            PaneNode::Pane { tab_id } => tab_id
                .parse::<usize>()
                .ok()
                .map_or_else(|| div().size_full(), |id| self.render_pane(id, source, cx)),
            PaneNode::Split {
                orientation,
                children,
                sizes,
            } => {
                let horizontal = *orientation == SplitOrientation::Horizontal;
                let count = children.len();
                let resolved = resolve_sizes(sizes, count);
                let minimum = PaneMinimum {
                    width: 220.,
                    height: 140.,
                };
                let minimums = children
                    .iter()
                    .map(|child| minimum_extent(child, *orientation, minimum, f64::from(sash::GAP)))
                    .collect();
                let sashes = sash::Sashes::new(
                    sash::SplitSpec {
                        tab: active,
                        path: &path,
                        orientation: *orientation,
                        sizes: &resolved,
                        minimums,
                        signature: get_pane_ids(node),
                    },
                    window,
                    cx,
                );
                let mut split = div().size_full().relative().flex().min_w_0().min_h_0();
                if !horizontal {
                    split = split.flex_col();
                }
                for (index, child) in children.iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(index);
                    let pane = self.render_node(child, child_path, active, source, window, cx);
                    split = split.child(
                        div()
                            .flex_basis(px(0.))
                            .flex_grow(resolved.get(index).copied().unwrap_or(0.) as f32)
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .child(pane),
                    );
                    if index + 1 < count {
                        split = split.child(sashes.divider(index, self.ui.theme));
                    }
                }
                split.child(sashes.tracker())
            }
        }
    }

    fn render_pane(&self, tab_id: usize, source: Option<usize>, cx: &mut Context<Self>) -> Div {
        let Some(tab) = self.tabs.get(tab_id) else {
            return div().size_full();
        };
        let owner = cx.entity_id();
        let workspace = cx.weak_entity();
        let mut pane = div()
            .debug_selector(|| format!("workspace-pane-{tab_id}"))
            .relative()
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(tab.view.clone())
            .children(tab.context_menu.clone())
            .children(
                tab.search_bar
                    .as_ref()
                    .filter(|search| search.read(cx).is_open())
                    .cloned(),
            )
            .capture_any_mouse_down(cx.listener(move |this, _, window, cx| {
                if this.tabs.active_id() != tab_id {
                    if this
                        .tabs
                        .active()
                        .search_bar
                        .as_ref()
                        .is_some_and(|bar| bar.read(cx).owns_pointer())
                    {
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    this.focus_pane(tab_id, window, cx);
                }
            }));
        if let Some(source) = source
            && self.splits.can_split(source, tab_id)
        {
            let primary = self.ui.theme.colors.primary;
            pane = pane.child(leaf_drop_surface(
                tab_id,
                hsla(primary, 0.9),
                hsla(primary, 0.18),
                move |drag, edge, window, cx| {
                    if drag.owner == owner && drag.id == source {
                        let _ = workspace.update(cx, |this, cx| {
                            this.split_tab(source, tab_id, edge, window, cx)
                        });
                    }
                },
            ));
        }
        pane
    }
}

fn resolve_sizes(sizes: &[f64], count: usize) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    let fallback = 1. / count as f64;
    let mut resolved = (0..count)
        .map(|index| {
            sizes
                .get(index)
                .copied()
                .filter(|size| size.is_finite() && *size > 0.)
                .unwrap_or(fallback)
        })
        .collect::<Vec<_>>();
    let total: f64 = resolved.iter().sum();
    if !total.is_finite() || total <= 0. {
        resolved.fill(fallback);
    }
    resolved
}
