//! Checked/unchecked switch primitive with a local theme and focus ring.

use super::surface::surface;
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::{IntoElement, ParentElement, Styled, div, px};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwitchState {
    pub checked: bool,
    pub focused: bool,
    pub disabled: bool,
}

pub fn switch(state: SwitchState, theme: Theme) -> impl IntoElement {
    let colors = theme.colors;
    let track = if state.checked {
        colors.primary
    } else {
        colors.input
    };
    let thumb_x = if state.checked { 16.0 } else { 0.0 };
    div()
        .relative()
        .flex()
        .items_center()
        .h(px(20.0))
        .w(px(36.0))
        .rounded(px(10.0))
        .opacity(if state.disabled { 0.5 } else { 1.0 })
        .child(surface(
            hsla(track, 1.0),
            gpui_kit::transparent_black(),
            0.0,
            state.focused.then_some((hsla(colors.ring, 0.5), 3.0)),
        ))
        .child(
            div()
                .absolute()
                .left(px(2.0 + thumb_x))
                .top(px(2.0))
                .size(px(16.0))
                .rounded(px(8.0))
                .bg(hsla(colors.background, 1.0)),
        )
}
