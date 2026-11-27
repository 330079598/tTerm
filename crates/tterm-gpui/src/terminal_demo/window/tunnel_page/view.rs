//! The read-only Port Forwarding page.
//!
//! The route controller owns loading, reloading and every tunnel action. This
//! view renders one immutable snapshot and deliberately offers no buttons for
//! operations it does not perform. The card, route and formatting details live
//! in child modules so the page shell stays easy to scan.

mod card;
mod format;
mod route;
#[cfg(test)]
mod visual_tests;

use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Div, Entity, FontWeight, Role, Stateful, div, px};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationKey, TranslationRequest};
use tterm_ui::components::icons::{WAYPOINTS, icon};
use tterm_ui::tokens::color::Hsl;
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use self::format::now_unix_ms;
use crate::locale::Locale;

use super::data::{TunnelPageModel, TunnelPageState};

/// The page root, usable by the route tests.
pub const TUNNEL_PAGE_ID: &str = "tunnel-page";
/// The bordered heading above the card list.
pub const TUNNEL_PAGE_HEADER_ID: &str = "tunnel-page-header";
/// The page title.
pub const TUNNEL_PAGE_TITLE_ID: &str = "tunnel-page-title";
/// The explanatory line under the title.
pub const TUNNEL_PAGE_DESCRIPTION_ID: &str = "tunnel-page-description";
/// The green count shown after the description while tunnels are running.
pub const TUNNEL_PAGE_RUNNING_COUNT_ID: &str = "tunnel-page-running-count";
/// The scrollable page body.
pub const TUNNEL_PAGE_CONTENT_ID: &str = "tunnel-page-content";
/// The card grid.
pub const TUNNEL_PAGE_LIST_ID: &str = "tunnel-page-list";
/// The visible loading state.
pub const TUNNEL_PAGE_LOADING_ID: &str = "tunnel-page-loading";
/// The visible load-failure state.
pub const TUNNEL_PAGE_ERROR_ID: &str = "tunnel-page-error";
/// The empty-state panel.
pub const TUNNEL_PAGE_EMPTY_ID: &str = "tunnel-page-empty";

const HEADER_PADDING_X: f32 = 24.0;
const HEADER_PADDING_Y: f32 = 15.0;
const CONTENT_PADDING: f32 = 24.0;
const EMPTY_MAX_WIDTH: f32 = 448.0;
const EMPTY_PADDING_Y: f32 = 64.0;

/// The entity the route log lists and the window renders.
pub struct TunnelPageView {
    locale: Entity<Locale>,
    theme: Theme,
}

impl TunnelPageView {
    /// Creates a view that reads translated labels from `locale`.
    pub fn new(locale: Entity<Locale>, theme: Theme) -> Self {
        Self { locale, theme }
    }

    /// Applies a theme without rebuilding the page entity.
    pub fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        cx.notify();
    }

    /// Renders one loading, failure, empty or tunnel-list snapshot.
    pub fn render_page(&self, model: &TunnelPageModel, cx: &mut Context<Self>) -> AnyElement {
        let colors = self.theme.colors;
        let now = now_unix_ms();
        div()
            .id(TUNNEL_PAGE_ID)
            .debug_selector(|| TUNNEL_PAGE_ID.to_string())
            .size_full()
            .flex()
            .flex_col()
            .bg(hsla(colors.background, 1.0))
            .text_color(hsla(colors.foreground, 1.0))
            .child(self.header(model, cx))
            .child(
                div()
                    .id(TUNNEL_PAGE_CONTENT_ID)
                    .debug_selector(|| TUNNEL_PAGE_CONTENT_ID.to_string())
                    .min_h(px(0.0))
                    .flex_1()
                    .overflow_y_scroll()
                    .p(px(CONTENT_PADDING))
                    .child(self.body(model, now, cx)),
            )
            .into_any_element()
    }

    fn header(&self, model: &TunnelPageModel, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let title = self.text(cx, keys::TUNNELS_TITLE);
        let description = self.text(cx, keys::TUNNELS_DESCRIPTION);
        let mut summary = div()
            .flex()
            .flex_wrap()
            .items_baseline()
            .gap(px(8.0))
            .mt(px(2.0))
            .text_size(px(14.0))
            .line_height(px(19.0))
            .text_color(hsla(colors.muted_foreground, 1.0))
            .child(description);
        if model.running_count > 0 {
            summary = summary.child(
                div()
                    .id(TUNNEL_PAGE_RUNNING_COUNT_ID)
                    .debug_selector(|| TUNNEL_PAGE_RUNNING_COUNT_ID.to_string())
                    .font_weight(FontWeight(500.0))
                    .text_color(hsla(colors.success, 1.0))
                    .child(self.text_count(
                        cx,
                        keys::TUNNELS_RUNNING_COUNT,
                        model.running_count as u64,
                    )),
            );
        }
        div()
            .id(TUNNEL_PAGE_HEADER_ID)
            .debug_selector(|| TUNNEL_PAGE_HEADER_ID.to_string())
            .flex()
            .items_start()
            .justify_between()
            .gap(px(16.0))
            .border_b(px(1.0))
            .border_color(hsla(colors.border, 0.8))
            .px(px(HEADER_PADDING_X))
            .py(px(HEADER_PADDING_Y))
            .child(
                div()
                    .min_w(px(0.0))
                    .child(
                        div()
                            .id(TUNNEL_PAGE_TITLE_ID)
                            .debug_selector(|| TUNNEL_PAGE_TITLE_ID.to_string())
                            .text_size(px(16.0))
                            .line_height(px(21.0))
                            .font_weight(FontWeight(600.0))
                            .child(title),
                    )
                    .child(
                        div()
                            .id(TUNNEL_PAGE_DESCRIPTION_ID)
                            .debug_selector(|| TUNNEL_PAGE_DESCRIPTION_ID.to_string())
                            .child(summary),
                    ),
            )
    }

    fn body(&self, model: &TunnelPageModel, now: u64, cx: &mut Context<Self>) -> AnyElement {
        match &model.state {
            TunnelPageState::Loading => self.loading(cx).into_any_element(),
            TunnelPageState::Error(message) => self.error(message, cx).into_any_element(),
            TunnelPageState::Ready if model.rules.is_empty() => self.empty(cx).into_any_element(),
            TunnelPageState::Ready => self.list(&model.rules, now, cx).into_any_element(),
        }
    }

    fn loading(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let label = self.text(cx, keys::COMMON_LOADING);
        div()
            .id(TUNNEL_PAGE_LOADING_ID)
            .debug_selector(|| TUNNEL_PAGE_LOADING_ID.to_string())
            .role(Role::Status)
            .aria_label(label.clone())
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .py(px(EMPTY_PADDING_Y))
            .text_size(px(14.0))
            .text_color(hsla(colors.muted_foreground, 1.0))
            .child(label)
    }

    fn error(&self, message: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let title = self.text(cx, keys::TUNNELS_LOAD_FAILED);
        let mut details = div()
            .text_size(px(14.0))
            .line_height(px(20.0))
            .text_color(hsla(colors.muted_foreground, 1.0))
            .text_center();
        if !message.is_empty() {
            details = details.child(message.to_string());
        }
        centered_message(TUNNEL_PAGE_ERROR_ID, title, colors.destructive, details).role(Role::Alert)
    }

    fn empty(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let title = self.text(cx, keys::TUNNELS_EMPTY_TITLE);
        let description = self.text(cx, keys::TUNNELS_EMPTY_DESCRIPTION_READ_ONLY);
        div()
            .id(TUNNEL_PAGE_EMPTY_ID)
            .debug_selector(|| TUNNEL_PAGE_EMPTY_ID.to_string())
            .w_full()
            .max_w(px(EMPTY_MAX_WIDTH))
            .mx_auto()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.0))
            .py(px(EMPTY_PADDING_Y))
            .text_center()
            .child(
                div()
                    .size(px(48.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(12.0))
                    .border(px(1.0))
                    .border_color(hsla(colors.border, 1.0))
                    .bg(hsla(colors.muted, 1.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(icon(WAYPOINTS, hsla(colors.muted_foreground, 1.0), 24.0)),
            )
            .child(
                div()
                    .text_size(px(16.0))
                    .line_height(px(22.0))
                    .font_weight(FontWeight(600.0))
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(description),
            )
    }

    fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.locale
            .read(cx)
            .text(key)
            .unwrap_or_else(|error| fallback_text(key, error))
    }

    fn text_count(&self, cx: &App, key: TranslationKey, count: u64) -> String {
        self.locale
            .read(cx)
            .translate(TranslationRequest::new(key).count(count as f64))
            .unwrap_or_else(|error| fallback_text(key, error))
    }

    fn text_arg(&self, cx: &App, key: TranslationKey, name: &'static str, value: &str) -> String {
        let arguments = [(name, ArgumentValue::Text(value))];
        self.locale
            .read(cx)
            .translate(TranslationRequest::new(key).arguments(&arguments))
            .unwrap_or_else(|error| fallback_text(key, error))
    }
}

fn fallback_text(key: TranslationKey, error: impl std::fmt::Display) -> String {
    eprintln!("tterm-gpui: failed to translate a tunnel page label: {error}");
    key.as_str().to_string()
}

fn centered_message(
    id: &'static str,
    title: String,
    title_color: Hsl,
    details: Div,
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .w_full()
        .max_w(px(EMPTY_MAX_WIDTH))
        .mx_auto()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.0))
        .py(px(EMPTY_PADDING_Y))
        .text_center()
        .child(
            div()
                .text_size(px(16.0))
                .line_height(px(22.0))
                .font_weight(FontWeight(600.0))
                .text_color(hsla(title_color, 1.0))
                .child(title),
        )
        .child(details)
}
