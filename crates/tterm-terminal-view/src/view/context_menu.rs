//! The host-owned context menu hook for a right-click inside the grid.
//!
//! The view reports the gesture and whether Copy has anything to copy. The
//! host owns the menu's labels, focus, dismissal and the actions, so this
//! crate keeps no dependency on a particular menu component.

use std::rc::Rc;

use super::TerminalView;
use gpui_kit::{Context, Pixels, Point, Window};

/// What the user right-clicked and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextMenuRequest {
    /// The window-space position of the click, which is also the menu origin.
    pub position: Point<Pixels>,
    /// Whether the selection had text when the click happened.
    pub has_selection: bool,
}

/// Opens the host's menu for this pane.
///
/// The handler runs while the view's own context is borrowed, so it must not
/// call back into the view; it records the target and opens the menu after.
pub type ContextMenuHandler =
    Rc<dyn Fn(ContextMenuRequest, &mut Window, &mut Context<TerminalView>)>;

impl TerminalView {
    /// Reports a right-click that should open the pane's context menu.
    pub(super) fn show_context_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(handler) = self.context_menu.clone() else {
            return;
        };
        let has_selection = self
            .terminal
            .selection_text()
            .is_some_and(|text| !text.is_empty());
        handler(
            ContextMenuRequest {
                position,
                has_selection,
            },
            window,
            cx,
        );
    }
}
