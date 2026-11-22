//! Connects the Port Forwarding page to the title bar.

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, Role, Stateful};
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::icons::{WAYPOINTS, icon};
use tterm_ui::tokens::gpui::hsla;

use super::OPEN_BUTTON_ID;
use crate::terminal_demo::window::Prototype;

impl Prototype {
    /// The Waypoints entry Tauri places before Settings in the title bar.
    pub(in crate::terminal_demo::window) fn tunnel_page_button(
        &self,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label = self.tunnel_page_title(cx);
        button(
            OPEN_BUTTON_ID,
            "",
            Variant::Ghost,
            ButtonSize::IconSm,
            ButtonState::default(),
            self.ui.theme,
        )
        .debug_selector(|| OPEN_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .aria_description(label)
        .child(icon(
            WAYPOINTS,
            hsla(self.ui.theme.colors.muted_foreground, 1.0),
            16.0,
        ))
        .on_click(cx.listener(|this, _, window, cx| this.open_tunnel_page(window, cx)))
    }
}
