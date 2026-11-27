//! The settings route on the window: open, close and the title-bar entry.
//!
//! Settings is a route above the workspace, not a tab. Opening it hides the
//! visible panes but keeps every tab, split and session alive, so closing it
//! returns to the exact pane that had focus.

use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, Stateful, Window};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_ui::components::icons::SETTINGS;

use super::super::Prototype;
use super::SETTINGS_OPEN_BUTTON_ID;
use crate::app_window::chrome::actions::action_button;

impl Prototype {
    /// Opens the settings route, or does nothing when it is already showing.
    pub(in super::super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_active_search(window, cx);
        if self.settings_open {
            self.ui
                .settings_page
                .update(cx, |page, cx| page.open(window, cx));
            return;
        }
        if self.tunnel_page_is_open() {
            self.close_tunnel_page(window, cx);
        }
        self.picker = None;
        self.ui
            .template_ui
            .update(cx, |template_ui, cx| template_ui.dismiss(cx));
        let previous = self.visible_ids();
        for id in previous {
            if let Some(tab) = self.tabs.get(id) {
                tab.leave(cx);
            }
        }
        self.ui
            .settings_page
            .update(cx, |page, cx| page.open(window, cx));
        self.settings_open = true;
        cx.notify();
    }

    /// Closes the settings route and restores the workspace's visible panes.
    ///
    /// Returns `false` while a settings write is in flight so the route does
    /// not discard a pending write.
    pub(in super::super) fn close_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.settings_open {
            return true;
        }
        let closed = self.ui.settings_page.update(cx, |page, cx| page.close(cx));
        if !closed {
            return false;
        }
        self.settings_open = false;
        self.refresh_panes(Vec::new(), window, cx);
        cx.notify();
        true
    }

    /// Whether the settings route owns the window content.
    pub(in super::super) fn settings_is_open(&self) -> bool {
        self.settings_open
    }

    /// The title-bar entry that opens settings.
    pub(in super::super) fn settings_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.settings_text(cx, keys::SETTINGS_OPEN);
        action_button(SETTINGS_OPEN_BUTTON_ID, label, SETTINGS, self.ui.theme)
            .on_click(cx.listener(|this, _, window, cx| this.toggle_settings(window, cx)))
    }

    /// The title-bar entry both opens and closes the singleton route, so a
    /// pointer user has the same way back as Escape.
    fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
            self.close_settings(window, cx);
        } else {
            self.open_settings(window, cx);
        }
    }

    /// Renders the route content; the title bar stays mounted above it.
    pub(in super::super) fn settings_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        self.ui
            .settings_page
            .update(cx, |page, cx| page.render_page(window, cx))
    }

    fn settings_text(&self, cx: &App, key: TranslationKey) -> String {
        self.ui.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a settings label: {error}");
            key.as_str().to_string()
        })
    }
}
