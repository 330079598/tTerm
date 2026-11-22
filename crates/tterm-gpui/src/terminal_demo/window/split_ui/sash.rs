//! Each split owns one drag state and measures its own room, so nested sashes
//! resize only the panes beside them.

use super::super::Prototype;
use gpui_kit::prelude::*;
use gpui_kit::{
    Context, CursorStyle, DispatchPhase, Div, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, Stateful, WeakEntity, Window, canvas, div, px,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tterm_core::workspace::{
    pane_layout::drag_sash,
    tab_groups::{PaneNode, SplitOrientation, get_pane_ids},
};
use tterm_ui::tokens::{gpui::hsla, theme::Theme};

pub(super) const GAP: f32 = 8.;

#[derive(Clone)]
struct Drag {
    origin: Pixels,
    index: usize,
    sizes: Vec<f64>,
    room: f64,
    minimums: Vec<f64>,
    signature: Vec<String>,
}

pub(super) struct SplitSpec<'a> {
    pub(super) tab: usize,
    pub(super) path: &'a [usize],
    pub(super) orientation: SplitOrientation,
    pub(super) sizes: &'a [f64],
    pub(super) minimums: Vec<f64>,
    pub(super) signature: Vec<String>,
}

/// The sash state shared by every divider in one split.
pub(super) struct Sashes {
    owner: WeakEntity<Prototype>,
    tab: usize,
    path: Vec<usize>,
    orientation: SplitOrientation,
    sizes: Vec<f64>,
    minimums: Vec<f64>,
    signature: Vec<String>,
    gesture: Rc<RefCell<Option<Drag>>>,
    room: Rc<Cell<f64>>,
    count: usize,
}

impl Sashes {
    pub(super) fn new(
        spec: SplitSpec<'_>,
        window: &mut Window,
        cx: &mut Context<Prototype>,
    ) -> Self {
        let key = format!("pane-sash-state-{}", path_key(spec.path));
        let gesture = window
            .use_keyed_state(key, cx, |_, _| Rc::new(RefCell::new(None::<Drag>)))
            .read(cx)
            .clone();
        let stale = gesture
            .borrow()
            .as_ref()
            .is_some_and(|drag| drag.signature != spec.signature);
        if stale {
            *gesture.borrow_mut() = None;
        }
        Self {
            owner: cx.weak_entity(),
            tab: spec.tab,
            path: spec.path.to_vec(),
            orientation: spec.orientation,
            sizes: spec.sizes.to_vec(),
            minimums: spec.minimums,
            signature: spec.signature,
            gesture,
            room: Rc::new(Cell::new(0.)),
            count: spec.sizes.len(),
        }
    }

    /// One hit target for the divider after `index`.
    pub(super) fn divider(&self, index: usize, theme: Theme) -> Stateful<Div> {
        let selector = selector(&self.path, index);
        let horizontal = self.orientation == SplitOrientation::Horizontal;
        let gesture = self.gesture.clone();
        let sizes = self.sizes.clone();
        let minimums = self.minimums.clone();
        let signature = self.signature.clone();
        let room = self.room.clone();
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .occlude()
            .when(horizontal, |sash| {
                sash.w(px(GAP))
                    .h_full()
                    .cursor(CursorStyle::ResizeLeftRight)
            })
            .when(!horizontal, |sash| {
                sash.h(px(GAP)).w_full().cursor(CursorStyle::ResizeUpDown)
            })
            .hover(|sash| sash.bg(hsla(theme.colors.foreground, 0.12)))
            .child(
                div()
                    .bg(hsla(theme.colors.border, 0.42))
                    .when(horizontal, |line| line.w(px(1.)).h_full())
                    .when(!horizontal, |line| line.h(px(1.)).w_full()),
            )
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                let origin = coordinate(horizontal, event.position);
                *gesture.borrow_mut() = Some(Drag {
                    origin,
                    index,
                    sizes: sizes.clone(),
                    room: room.get(),
                    minimums: minimums.clone(),
                    signature: signature.clone(),
                });
                window.prevent_default();
                cx.stop_propagation();
            })
    }

    /// Measures the split and keeps the drag alive beyond the divider bounds.
    pub(super) fn tracker(&self) -> impl IntoElement + use<> {
        let horizontal = self.orientation == SplitOrientation::Horizontal;
        let measured = self.room.clone();
        let gesture = self.gesture.clone();
        let release = self.gesture.clone();
        let owner = self.owner.clone();
        let release_owner = owner.clone();
        let tab = self.tab;
        let path = self.path.clone();
        let span = GAP * (self.count.saturating_sub(1) as f32);
        canvas(
            move |bounds, _, _| {
                let extent = if horizontal {
                    bounds.size.width
                } else {
                    bounds.size.height
                };
                measured.set((f32::from(extent) - span).max(0.) as f64);
            },
            move |_, (), window, _| {
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let Some(drag) = gesture.borrow().clone() else {
                        return;
                    };
                    if event.pressed_button != Some(MouseButton::Left) {
                        *gesture.borrow_mut() = None;
                        return;
                    }
                    let delta = f64::from(f32::from(
                        coordinate(horizontal, event.position) - drag.origin,
                    ));
                    let current = owner
                        .update(cx, |workspace, _| {
                            workspace
                                .splits
                                .root(tab)
                                .and_then(|root| split_signature(root, &path))
                        })
                        .ok()
                        .flatten();
                    if current.as_ref() != Some(&drag.signature) {
                        *gesture.borrow_mut() = None;
                        return;
                    }
                    let sizes = drag_sash(
                        &drag.sizes,
                        drag.index as i64,
                        delta,
                        drag.room,
                        &drag.minimums,
                    );
                    let _ = owner.update(cx, |workspace, cx| {
                        if workspace.splits.resize(tab, &path, &sizes) {
                            cx.notify();
                        }
                    });
                    cx.stop_propagation();
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture
                        && event.button == MouseButton::Left
                        && release.borrow_mut().take().is_some()
                    {
                        let _ = release_owner.update(cx, |workspace, cx| {
                            workspace.mark_workspace_dirty(cx);
                        });
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .inset_0()
        .size_full()
    }
}

fn coordinate(horizontal: bool, point: Point<Pixels>) -> Pixels {
    if horizontal { point.x } else { point.y }
}

fn path_key(path: &[usize]) -> String {
    path.iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn selector(path: &[usize], index: usize) -> String {
    if path.is_empty() {
        if index == 0 {
            return "workspace-sash".into();
        }
        return format!("workspace-sash-root-{index}");
    }
    let path = path
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join("-");
    if index == 0 {
        format!("workspace-sash-{path}")
    } else {
        format!("workspace-sash-{path}-divider-{index}")
    }
}

fn split_signature(root: &PaneNode, path: &[usize]) -> Option<Vec<String>> {
    let mut node = root;
    for index in path {
        let PaneNode::Split { children, .. } = node else {
            return None;
        };
        node = children.get(*index)?;
    }
    Some(get_pane_ids(node))
}

#[cfg(test)]
mod tests {
    use super::selector;

    #[test]
    fn sash_selectors_are_distinct_for_root_and_nested_paths() {
        assert_eq!(selector(&[], 0), "workspace-sash");
        assert_eq!(selector(&[], 1), "workspace-sash-root-1");
        assert_eq!(selector(&[1], 0), "workspace-sash-1");
        assert_eq!(selector(&[1, 0], 0), "workspace-sash-1-0");
        assert_eq!(selector(&[1], 1), "workspace-sash-1-divider-1");
        assert_eq!(selector(&[1, 0], 1), "workspace-sash-1-0-divider-1");
        assert_eq!(selector(&[1, 1], 0), "workspace-sash-1-1");
    }
}
