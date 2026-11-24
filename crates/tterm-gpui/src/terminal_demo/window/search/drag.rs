//! Dragging the search bar inside its pane.
//!
//! Only the handle starts a gesture, only the left button carries it, and the
//! bar stays eight logical pixels inside the pane from the view's last frame.
//! In a window 640 points wide or narrower the bar fills the pane between two
//! ten-pixel gutters initially, then uses the same eight-pixel drag margin. The gesture ends on
//! release, on a pane resize, and on a window move or resize; it never touches
//! the terminal model or the PTY. When geometry cancels a gesture while the
//! pointer is still down, the remaining motion and the release are swallowed so
//! that mode 1003 reporting never sees them.

use gpui_kit::{
    Bounds, Context, CursorStyle, DispatchPhase, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollEvent, Size, Styled as _, Window, canvas,
    point, px,
};

use super::SearchBar;

/// The inset the dragged bar keeps from every pane edge, as the React hook does.
pub(super) const BAR_MARGIN: f32 = 8.0;
/// The initial top offset of the bar inside the pane.
pub(super) const BAR_DEFAULT_TOP: f32 = 12.0;
/// The initial right offset of the bar inside the pane.
pub(super) const BAR_DEFAULT_RIGHT: f32 = 14.0;
/// The bar's `p(7px)` padding.
pub(super) const BAR_PADDING: f32 = 7.0;
/// The bar's 1px border.
pub(super) const BAR_BORDER: f32 = 1.0;
/// The outer width the production controls add up to at 100%.
pub(super) const BAR_WIDTH: f32 = 522.0;
/// The grip's hit target, matching `.terminal-search-drag-handle`.
pub(super) const HANDLE_WIDTH: f32 = 20.0;
/// The grip's height, matching `.terminal-search-drag-handle`.
pub(super) const HANDLE_HEIGHT: f32 = 26.0;
/// The widest window whose bar fills the pane and wraps.
pub(super) const NARROW_WINDOW: f32 = 640.0;
/// The initial gutter between the bar and a narrow window's pane edges.
pub(super) const NARROW_INSET: f32 = 10.0;
/// The narrowest pane that still fits the fixed-width bar between its margins.
const BAR_MIN_SURFACE: f32 = BAR_WIDTH + 2.0 * BAR_MARGIN;

/// The outer width the bar requests for one pane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BarWidth {
    /// A fixed outer width, used by the 522 point bar and by a narrow pane.
    Fixed(Pixels),
    /// Stretch between two [`NARROW_INSET`] gutters.
    Fill,
}

/// The bar width for the current window and pane.
///
/// A narrow pane inside a wide window still gets a fixed, draggable width; only
/// a genuinely narrow window fills the pane edge to edge.
pub(super) fn bar_width(window_width: f32, surface_width: Option<f32>) -> BarWidth {
    if window_width <= NARROW_WINDOW {
        return BarWidth::Fill;
    }
    match surface_width {
        Some(width) if width < BAR_MIN_SURFACE => {
            BarWidth::Fixed(px((width - 2.0 * BAR_MARGIN).max(1.0)))
        }
        _ => BarWidth::Fixed(px(BAR_WIDTH)),
    }
}

/// The inset the bar keeps from the pane edges in the current window.
pub(super) fn inset(window_width: f32) -> f32 {
    if window_width <= NARROW_WINDOW {
        NARROW_INSET
    } else {
        BAR_MARGIN
    }
}

/// One left-button drag from the grip.
#[derive(Clone, Debug)]
pub(super) struct SearchDrag {
    pointer: Point<Pixels>,
    origin: Point<Pixels>,
    surface: Bounds<Pixels>,
    bar: Size<Pixels>,
}

impl SearchBar {
    /// A grip press owns all pointer input until its release, even after a resize.
    pub(in super::super) fn owns_pointer(&self) -> bool {
        self.drag.is_some() || self.suppress_mouse_until_release
    }

    /// The grip cursor for the current gesture.
    pub(super) fn handle_cursor(&self) -> CursorStyle {
        if self.drag.is_some() {
            CursorStyle::ClosedHand
        } else {
            CursorStyle::OpenHand
        }
    }

    /// The bar's origin and the pane rectangle it is clamped to.
    ///
    /// `None` before the first measured bar; the caller then keeps the CSS
    /// default anchor. The result is recomputed from the latest measurements on
    /// every call, so a pane or window resize re-clamps without a drag.
    pub(super) fn placement(
        &self,
        window_width: f32,
    ) -> Option<(Point<Pixels>, Bounds<Pixels>, Size<Pixels>)> {
        place(
            window_width,
            self.surface_bounds.get(),
            self.bar_size.get(),
            self.position,
        )
    }

    /// Starts a gesture from the grip. Other buttons and the bar body do nothing.
    pub(super) fn start_drag(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }
        let window_width = f32::from(window.bounds().size.width);
        let Some((origin, surface, bar)) = self.placement(window_width) else {
            return;
        };
        self.drag = Some(SearchDrag {
            pointer: event.position,
            origin,
            surface,
            bar,
        });
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    /// Moves the bar with the pointer, or ends the gesture on a stale surface.
    pub(super) fn drag_moved(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.suppress_mouse_until_release {
            if event.pressed_button.is_none() {
                self.suppress_mouse_until_release = false;
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        let Some(drag) = self.drag.clone() else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            if event.pressed_button.is_none() {
                self.end_drag(cx);
            }
            cx.stop_propagation();
            return;
        }
        let Some(surface) = self
            .view
            .upgrade()
            .and_then(|view| view.read(cx).bounds())
            .or_else(|| self.surface_bounds.get())
        else {
            self.cancel_geometry(cx);
            cx.stop_propagation();
            return;
        };
        if surface != drag.surface {
            self.cancel_geometry(cx);
            cx.stop_propagation();
            return;
        }
        let delta = event.position - drag.pointer;
        let requested = point(drag.origin.x + delta.x, drag.origin.y + delta.y);
        let clamped = clamp_position(requested, surface.size, drag.bar, BAR_MARGIN);
        if self.position != Some(clamped) {
            self.position = Some(clamped);
            cx.notify();
        }
        cx.stop_propagation();
    }

    /// Ends the gesture; a later pointer move must not move the bar.
    pub(super) fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            cx.notify();
        }
    }

    /// Drops the gesture because its pane or window geometry moved.
    ///
    /// The pointer is still down, so the bar is returned to its default anchor
    /// and every later mouse event is swallowed until the release. Otherwise
    /// the terminal would see the end of a drag it never started and mode 1003
    /// would report that motion to the PTY.
    pub(super) fn cancel_geometry(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            self.suppress_mouse_until_release = true;
            self.position = None;
            cx.notify();
        }
    }

    /// Registers the window-level move and release listeners for one frame.
    ///
    /// A one-pixel canvas keeps the callbacks alive while the pointer is
    /// outside the bar. It has no mouse handlers of its own, so the bar's
    /// controls and the terminal keep their hit testing.
    pub(super) fn drag_tracker(&self, cx: &Context<Self>) -> impl IntoElement {
        let bar = cx.entity().downgrade();
        canvas(
            |_, _, _| (),
            move |_, (), window, _cx| {
                let moved = bar.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let _ = moved.update(cx, |bar, cx| bar.drag_moved(event, cx));
                });
                let released = bar.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let _ = released.update(cx, |bar, cx| {
                        if !bar.owns_pointer() {
                            return;
                        }
                        if event.button == MouseButton::Left {
                            bar.drag = None;
                            bar.suppress_mouse_until_release = false;
                            cx.notify();
                        }
                        cx.stop_propagation();
                    });
                });
                window.on_mouse_event(move |_: &MouseDownEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                });
                window.on_mouse_event(move |_: &ScrollEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture {
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .top(px(BAR_PADDING))
        .left(px(BAR_PADDING))
        .w(px(1.))
        .h(px(1.))
    }
}

/// The clamped origin for one pane.
///
/// The requested origin only sets the top-left corner; the pane size and the
/// measured bar keep it inside the margin. A missing measurement keeps the
/// caller's CSS default.
pub(super) fn place(
    window_width: f32,
    surface: Option<Bounds<Pixels>>,
    bar: Option<Size<Pixels>>,
    requested: Option<Point<Pixels>>,
) -> Option<(Point<Pixels>, Bounds<Pixels>, Size<Pixels>)> {
    let surface = surface?;
    let bar = bar?;
    let edge = inset(window_width);
    let surface_width = f32::from(surface.size.width);
    let default = match bar_width(window_width, Some(surface_width)) {
        BarWidth::Fill => point(px(edge), px(edge)),
        BarWidth::Fixed(_) => point(
            px((surface_width - BAR_DEFAULT_RIGHT - f32::from(bar.width)).max(edge)),
            px(BAR_DEFAULT_TOP),
        ),
    };
    let origin = clamp_position(requested.unwrap_or(default), surface.size, bar, BAR_MARGIN);
    Some((origin, surface, bar))
}

fn clamp_position(
    position: Point<Pixels>,
    surface: Size<Pixels>,
    bar: Size<Pixels>,
    inset: f32,
) -> Point<Pixels> {
    let max_x = (f32::from(surface.width) - f32::from(bar.width) - inset).max(inset);
    let max_y = (f32::from(surface.height) - f32::from(bar.height) - inset).max(inset);
    point(
        px(f32::from(position.x).clamp(inset, max_x)),
        px(f32::from(position.y).clamp(inset, max_y)),
    )
}
