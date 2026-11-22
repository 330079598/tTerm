//! The singleton settings route.
//!
//! Settings is a route above the workspace rather than a tab. Opening it
//! hides the visible panes but keeps every tab, split and session alive, so
//! closing it returns to the exact pane that had focus.

use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, FontWeight, Role, Stateful, Window, div, px};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::tokens::gpui::hsla;

use super::Prototype;

pub(super) const SETTINGS_PAGE_ID: &str = "settings-page";
pub(super) const SETTINGS_OPEN_BUTTON_ID: &str = "settings-page-open";
pub(super) const SETTINGS_BACK_BUTTON_ID: &str = "settings-page-back";

const PAGE_PADDING: f32 = 28.0;
const SECTION_WIDTH: f32 = 560.0;

impl Prototype {
    /// Opens the settings route, or does nothing when it is already showing.
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
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
            .scrollback_setting
            .update(cx, |setting, cx| setting.open_page(window, cx));
        self.settings_open = true;
        cx.notify();
    }

    /// Closes the settings route and restores the workspace's visible panes.
    ///
    /// Returns `false` while the scrollback setting is being saved so the
    /// route does not discard a pending write.
    pub(super) fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.settings_open {
            return true;
        }
        let closed = self
            .ui
            .scrollback_setting
            .update(cx, |setting, cx| setting.close_page(cx));
        if !closed {
            return false;
        }
        self.settings_open = false;
        self.refresh_panes(Vec::new(), window, cx);
        cx.notify();
        true
    }

    /// Whether the settings route owns the window content.
    pub(super) fn settings_is_open(&self) -> bool {
        self.settings_open
    }

    /// Sets the scrollback field for a route-level test.
    #[cfg(test)]
    pub(super) fn set_settings_scrollback_value(
        &mut self,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ui.scrollback_setting.update(cx, |setting, cx| {
            setting.set_value(value, window, cx);
        });
    }

    /// Submits the scrollback field for a route-level test.
    #[cfg(test)]
    pub(super) fn submit_settings_scrollback(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ui
            .scrollback_setting
            .update(cx, |setting, cx| setting.submit(window, cx));
    }

    /// The title-bar entry that opens settings.
    pub(super) fn settings_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.settings_text(cx, keys::SETTINGS_OPEN);
        button(
            SETTINGS_OPEN_BUTTON_ID,
            "⚙",
            Variant::Ghost,
            ButtonSize::Sm,
            ButtonState::default(),
            self.ui.theme,
        )
        .debug_selector(|| SETTINGS_OPEN_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(label)
        .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx)))
    }

    /// Renders the route content; the title bar stays mounted above it.
    pub(super) fn settings_page(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let title = self.settings_text(cx, keys::SETTINGS_TITLE);
        let back = self.settings_text(cx, keys::SETTINGS_BACK);
        let colors = self.ui.theme.colors;
        let back_button = button(
            SETTINGS_BACK_BUTTON_ID,
            back.clone(),
            Variant::Outline,
            ButtonSize::Sm,
            ButtonState::default(),
            self.ui.theme,
        )
        .debug_selector(|| SETTINGS_BACK_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(back)
        .on_click(cx.listener(|this, _, window, cx| {
            this.close_settings(window, cx);
        }));
        let section = self
            .ui
            .scrollback_setting
            .update(cx, |setting, cx| setting.section(cx));
        div()
            .id(SETTINGS_PAGE_ID)
            .debug_selector(|| SETTINGS_PAGE_ID.to_string())
            .size_full()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .p(px(PAGE_PADDING))
            .text_color(hsla(colors.foreground, 1.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(back_button)
                    .child(
                        div()
                            .text_size(px(20.0))
                            .font_weight(FontWeight(600.0))
                            .child(title),
                    ),
            )
            .child(div().w_full().max_w(px(SECTION_WIDTH)).child(section))
            .into_any_element()
    }

    fn settings_text(&self, cx: &App, key: TranslationKey) -> String {
        self.ui.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a settings label: {error}");
            key.as_str().to_string()
        })
    }
}
