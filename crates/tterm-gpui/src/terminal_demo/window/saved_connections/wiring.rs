//! Connects the saved-profile picker to the title bar and workspace routes.

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, Stateful, Window};
use tterm_core::i18n::keys;
use tterm_ui::components::icons::BOOK_MARKED;

use super::OPEN_BUTTON_ID;
use crate::app_window::chrome::actions::action_button;
use crate::terminal_demo::window::Prototype;

impl Prototype {
    /// Opens the singleton picker above the live workspace.
    pub(in crate::terminal_demo::window) fn open_saved_connections(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.picker = None;
        self.ui
            .template_ui
            .update(cx, |template_ui, cx| template_ui.dismiss(cx));
        if self.tunnel_page_is_open() {
            self.close_tunnel_page(window, cx);
        }
        if self.settings_is_open() && !self.close_settings(window, cx) {
            return;
        }
        self.ui
            .saved_connections
            .update(cx, |saved_connections, cx| {
                saved_connections.open(window, cx);
            });
    }

    /// The title-bar entry for saved SSH connections.
    pub(in crate::terminal_demo::window) fn saved_connections_button(
        &self,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let saved_connections = self.ui.saved_connections.read(cx);
        let label = saved_connections.text(cx, keys::PROFILES_TITLE);
        action_button(OPEN_BUTTON_ID, label, BOOK_MARKED, self.ui.theme)
            .on_click(cx.listener(|this, _, window, cx| this.open_saved_connections(window, cx)))
    }
}
