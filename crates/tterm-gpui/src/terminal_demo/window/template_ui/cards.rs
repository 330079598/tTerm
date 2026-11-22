//! Picker cards shared by built-in and saved templates.

use gpui_kit::prelude::*;
use gpui_kit::{Div, Role, Stateful, div, px};
use tterm_core::workspace::pane_templates::LayoutShape;
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use super::preview::layout_preview;

const CARD_WIDTH: f32 = 176.0;
const CARD_MIN_HEIGHT: f32 = 98.0;
const CARD_PADDING: f32 = 8.0;
const CARD_RADIUS: f32 = 7.0;

/// The data one card renders. The caller owns the click behavior.
pub(super) struct CardSpec<'a> {
    pub id: String,
    pub selector: String,
    pub label: String,
    pub pane_count: String,
    pub shape: &'a LayoutShape,
    pub disabled: bool,
    pub needs_session: Option<String>,
}

/// Builds a card body without deciding what activating it does.
pub(super) fn card(spec: CardSpec<'_>, theme: Theme) -> Stateful<Div> {
    let colors = theme.colors;
    let CardSpec {
        id,
        selector,
        label,
        pane_count,
        shape,
        disabled,
        needs_session,
    } = spec;
    div()
        .id(id)
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(label.clone())
        .when_some(needs_session, |card, message| {
            card.aria_description(message)
        })
        .w(px(CARD_WIDTH))
        .min_h(px(CARD_MIN_HEIGHT))
        .p(px(CARD_PADDING))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(6.0))
        .rounded(px(CARD_RADIUS))
        .border(px(1.0))
        .border_color(hsla(colors.border, 1.0))
        .bg(hsla(colors.card, 1.0))
        .text_color(hsla(colors.card_foreground, 1.0))
        .when(disabled, |card| card.opacity(0.5))
        .when(!disabled, |card| {
            card.cursor_pointer()
                .hover(|style| style.bg(hsla(colors.accent, 0.75)))
        })
        .child(layout_preview(shape, theme))
        .child(
            div()
                .w_full()
                .min_w_0()
                .truncate()
                .text_center()
                .text_size(px(13.0))
                .font_weight(gpui_kit::FontWeight(500.0))
                .child(label),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(hsla(colors.muted_foreground, 1.0))
                .child(pane_count),
        )
}
