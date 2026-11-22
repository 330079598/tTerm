//! A tab drop moves the existing terminal view beside another session.

use std::{cell::Cell, rc::Rc};

use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, Context, Div, Hsla, Pixels, Point, Window, canvas, div, fill, outline, px,
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

fn drop_surface(
    foreground: Hsla,
    background: Hsla,
    commit: impl Fn(&TabDrag, Edge, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let measured = Rc::new(Cell::new(None::<Bounds<Pixels>>));
    let target = measured.clone();
    div()
        .id("split-drop-surface")
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
        let selected = self.tabs.active_id();
        let pair = self.splits.visible_pair(selected);
        let mut root = div().size_full().relative().flex().min_w_0().min_h_0();
        if let Some(pair) = pair {
            let panes = [(pair.first, pair.ratio), (pair.second, 1. - pair.ratio)]
                .into_iter()
                .filter_map(|(id, weight)| {
                    let view = self.tabs.get(id)?.view.clone();
                    Some(
                        div()
                            .flex_basis(px(0.))
                            .flex_grow(weight)
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .child(view)
                            .capture_any_mouse_down(cx.listener(move |this, _, window, cx| {
                                if this.tabs.active_id() != id {
                                    this.focus_pane(id, window, cx);
                                }
                            })),
                    )
                })
                .collect::<Vec<_>>();
            return sash::layout(root, pair, panes.into_iter(), self.ui.theme, window, cx);
        }
        root = root.child(self.tabs.active().view.clone());
        if let Some((source, _)) = self.drag_layout.gesture.get()
            && cx.has_active_drag()
            && self.tabs.get(source).is_some()
            && self.splits.can_split(source, selected)
        {
            let owner = cx.entity_id();
            let workspace = cx.weak_entity();
            let primary = self.ui.theme.colors.primary;
            root = root.child(drop_surface(
                hsla(primary, 0.9),
                hsla(primary, 0.18),
                move |drag, edge, window, cx| {
                    if drag.owner == owner && drag.id == source {
                        let _ = workspace.update(cx, |this, cx| {
                            this.split_tab(source, selected, edge, window, cx)
                        });
                    }
                },
            ));
        }
        root
    }
}
