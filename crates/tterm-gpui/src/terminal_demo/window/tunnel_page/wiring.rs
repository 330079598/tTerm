//! Connects the Port Forwarding page to the title bar.

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, Stateful};
use tterm_ui::components::icons::WAYPOINTS;

use super::OPEN_BUTTON_ID;
use crate::app_window::chrome::actions::action_button;
use crate::terminal_demo::window::Prototype;

impl Prototype {
    /// The Waypoints entry Tauri places before Settings in the title bar.
    pub(in crate::terminal_demo::window) fn tunnel_page_button(
        &self,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label = self.tunnel_page_title(cx);
        action_button(OPEN_BUTTON_ID, label, WAYPOINTS, self.ui.theme)
            .on_click(cx.listener(|this, _, window, cx| this.open_tunnel_page(window, cx)))
    }
}
