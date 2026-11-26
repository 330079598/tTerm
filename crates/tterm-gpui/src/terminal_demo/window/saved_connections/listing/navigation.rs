//! Keyboard focus and scroll position for the saved-connection listing.
//!
//! The tracked scroll element has one direct child: the whole listing content.
//! Its nested group tree means `ScrollHandle::scroll_to_item` cannot address a
//! row, so each row reports the bounds it was laid out at and the selection
//! scrolls by the smallest offset that brings those bounds into the viewport.
//! Measurement records bounds; scrolling happens when the selection changes
//! and once after that row is measured, never on every frame.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::base::active_focus_trap;
use gpui_kit::{App, Bounds, FocusHandle, Pixels, ScrollHandle, Window, point, px};

/// The focus handles and measured bounds the listing owns across frames.
#[derive(Default)]
pub(in crate::terminal_demo::window::saved_connections) struct ListingNavigation {
    /// One tab stop per sort choice, keyed by its stable element id.
    sort_focus: RefCell<HashMap<&'static str, FocusHandle>>,
    /// Row bounds in the listing's content coordinates, without its scroll offset.
    row_bounds: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    /// The row the last selection change wants to keep visible. Its next
    /// measurement verifies the request against the layout that actually ran.
    pending: Rc<RefCell<Option<String>>>,
}

impl ListingNavigation {
    /// The tab stop for one By group / Recently used choice.
    pub(in crate::terminal_demo::window::saved_connections) fn sort_focus(
        &self,
        id: &'static str,
        cx: &App,
    ) -> FocusHandle {
        if let Some(handle) = self.sort_focus.borrow().get(id) {
            return handle.clone();
        }
        let handle = cx.focus_handle().tab_stop(true);
        self.sort_focus.borrow_mut().insert(id, handle.clone());
        handle
    }

    /// Forgets row positions from a previous opening.
    pub(in crate::terminal_demo::window::saved_connections) fn clear_rows(&self) {
        self.row_bounds.borrow_mut().clear();
        self.pending.borrow_mut().take();
    }

    /// Whether the keyboard is on one of the two sort choices.
    pub(in crate::terminal_demo::window::saved_connections) fn sort_control_focused(
        &self,
        window: &Window,
    ) -> bool {
        self.sort_focus
            .borrow()
            .values()
            .any(|handle| handle.is_focused(window))
    }

    /// The measurement listener for one profile row. The wrapper div reports
    /// its child's laid-out bounds, which include the current scroll offset;
    /// subtracting that offset keeps the stored bounds stable while scrolling.
    pub(in crate::terminal_demo::window::saved_connections) fn row_measurement(
        &self,
        id: &str,
        scroll_handle: &ScrollHandle,
    ) -> impl Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App) + 'static {
        let row_bounds = Rc::clone(&self.row_bounds);
        let pending = Rc::clone(&self.pending);
        let id = id.to_string();
        let scroll_handle = scroll_handle.clone();
        move |children: Vec<Bounds<Pixels>>, window: &mut Window, cx: &mut App| {
            let Some(row) = children.first().copied() else {
                return;
            };
            let bounds = Bounds::new(row.origin - scroll_handle.offset(), row.size);
            row_bounds.borrow_mut().insert(id.clone(), bounds);
            if pending.borrow().as_deref() != Some(id.as_str()) {
                return;
            }
            pending.borrow_mut().take();
            let scroll_handle = scroll_handle.clone();
            window.defer(cx, move |window, _cx| {
                if reveal_bounds(bounds, &scroll_handle) {
                    window.refresh();
                }
            });
        }
    }

    /// Scrolls the smallest amount that brings `id` inside the viewport.
    ///
    /// A row that is already visible is left alone. When the row is below the
    /// viewport its bottom edge becomes the nearest edge; above the viewport
    /// its top edge does.
    pub(in crate::terminal_demo::window::saved_connections) fn reveal(
        &self,
        id: &str,
        scroll_handle: &ScrollHandle,
    ) {
        self.pending.replace(Some(id.to_string()));
        let bounds = self.row_bounds.borrow().get(id).copied();
        if let Some(bounds) = bounds {
            reveal_bounds(bounds, scroll_handle);
        }
    }
}

fn reveal_bounds(bounds: Bounds<Pixels>, scroll_handle: &ScrollHandle) -> bool {
    let viewport = scroll_handle.bounds();
    if viewport.size.height <= px(0.0) {
        return false;
    }
    let offset = scroll_handle.offset();
    let top = bounds.top() + offset.y;
    let bottom = bounds.bottom() + offset.y;
    let delta = if top < viewport.top() {
        viewport.top() - top
    } else if bottom > viewport.bottom() {
        viewport.bottom() - bottom
    } else {
        return false;
    };
    let next = (offset.y + delta).clamp(-scroll_handle.max_offset().y, px(0.0));
    if next == offset.y {
        return false;
    }
    scroll_handle.set_offset(point(offset.x, next));
    true
}

/// Moves focus to the next or previous tab stop without letting a modal's
/// focus trap escape when no stop inside it can be reached.
pub(in crate::terminal_demo::window::saved_connections) fn step_focus(
    window: &mut Window,
    cx: &mut App,
    forward: bool,
) {
    let advance = |window: &mut Window, cx: &mut App| {
        if forward {
            window.focus_next(cx);
        } else {
            window.focus_prev(cx);
        }
    };
    let Some(container) = active_focus_trap(window, cx) else {
        advance(window, cx);
        return;
    };
    let mut visited = Vec::new();
    loop {
        advance(window, cx);
        if container.contains_focused(window, cx) {
            return;
        }
        let Some(focused) = window.focused(cx) else {
            break;
        };
        if visited.contains(&focused) {
            break;
        }
        visited.push(focused);
    }
    container.focus(window, cx);
}
