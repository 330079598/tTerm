//! Window capture listeners keep resizing when the pointer crosses a live terminal.
use super::super::{Prototype, split_state::Pair};
use gpui_kit::prelude::*;
use gpui_kit::{
    Context, CursorStyle, DispatchPhase, Div, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, Window, canvas, div, px,
};
use std::{cell::Cell, rc::Rc};
use tterm_core::workspace::{pane_layout::drag_sash, tab_groups::SplitOrientation};
use tterm_ui::tokens::{gpui::hsla, theme::Theme};

const GAP: f32 = 8.;

#[derive(Clone, Copy)]
struct Drag {
    origin: Pixels,
    base: f32,
    room: f64,
}

pub(super) fn layout(
    root: Div,
    pair: Pair,
    mut panes: impl Iterator<Item = Div>,
    theme: Theme,
    window: &mut Window,
    cx: &mut Context<Prototype>,
) -> Div {
    let horizontal = pair.axis == SplitOrientation::Horizontal;
    let coordinate = move |point: Point<Pixels>| if horizontal { point.x } else { point.y };
    let state = window.use_keyed_state(("pane-sash", pair.first), cx, |_, _| {
        Rc::new(Cell::new(None::<Drag>))
    });
    let gesture = state.read(cx).clone();
    let press = gesture.clone();
    let release = gesture.clone();
    let room = Rc::new(Cell::new(0.));
    let measured = room.clone();
    let owner = cx.weak_entity();
    let tracking = canvas(
        move |bounds, _, _| {
            measured.set(f64::from(
                f32::from(if horizontal {
                    bounds.size.width
                } else {
                    bounds.size.height
                }) - GAP,
            ));
        },
        move |_, (), window, _| {
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let Some(drag) = gesture.get() else { return };
                if event.pressed_button != Some(MouseButton::Left) {
                    gesture.set(None);
                    return;
                }
                let delta = f64::from(f32::from(coordinate(event.position) - drag.origin));
                let base = f64::from(drag.base);
                let min = if horizontal { 220. } else { 140. };
                let sizes = drag_sash(&[base, 1. - base], 0, delta, drag.room, &[min, min]);
                if let Some(first) = sizes.first() {
                    let _ = owner.update(cx, |workspace, cx| {
                        if workspace.splits.resize(pair.first, *first as f32) {
                            cx.notify();
                        }
                    });
                }
                cx.stop_propagation();
            });
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture
                    && event.button == MouseButton::Left
                    && release.take().is_some()
                {
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .inset_0()
    .size_full();
    let sash = div()
        .id("workspace-sash")
        .debug_selector(|| "workspace-sash".into())
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .occlude()
        .when(horizontal, |s| {
            s.w(px(GAP)).h_full().cursor(CursorStyle::ResizeLeftRight)
        })
        .when(!horizontal, |s| {
            s.h(px(GAP)).w_full().cursor(CursorStyle::ResizeUpDown)
        })
        .hover(|s| s.bg(hsla(theme.colors.foreground, 0.12)))
        .child(
            div()
                .bg(hsla(theme.colors.border, 0.42))
                .when(horizontal, |line| line.w(px(1.)).h_full())
                .when(!horizontal, |line| line.h(px(1.)).w_full()),
        )
        .on_mouse_down(MouseButton::Left, move |e, window, cx| {
            press.set(Some(Drag {
                origin: coordinate(e.position),
                base: pair.ratio,
                room: room.get(),
            }));
            window.prevent_default();
            cx.stop_propagation();
        });
    root.when(!horizontal, |r| r.flex_col())
        .children(panes.next())
        .child(sash)
        .children(panes.next())
        .child(tracking)
}
