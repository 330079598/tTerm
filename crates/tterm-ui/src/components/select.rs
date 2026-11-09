//! A native-looking select with a GPUI popup for the non-native client.

use super::icons::{CHEVRON_DOWN, icon};
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::{IntoElement, ParentElement, Styled, div, px};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectState {
    pub options: Vec<String>,
    pub selected: usize,
    pub open: bool,
    pub focused: bool,
    pub disabled: bool,
}

impl SelectState {
    pub fn new(options: Vec<String>) -> Self {
        Self {
            options,
            selected: 0,
            open: false,
            focused: false,
            disabled: false,
        }
    }
}

pub fn select(snapshot: &SelectState, theme: Theme) -> impl IntoElement {
    let colors = theme.colors;
    let border = if snapshot.focused {
        colors.ring
    } else {
        colors.input
    };
    let value = snapshot
        .options
        .get(snapshot.selected)
        .cloned()
        .unwrap_or_default();
    let button = div()
        .h(px(36.0))
        .w_full()
        .flex()
        .items_center()
        .px(px(12.0))
        .pr(px(36.0))
        .rounded(px(6.0))
        .border(px(1.0))
        .border_color(hsla(border, 1.0))
        .bg(hsla(colors.background, 1.0))
        .text_color(hsla(colors.foreground, 1.0))
        .opacity(if snapshot.disabled { 0.5 } else { 1.0 })
        .child(value)
        .child(div().absolute().right(px(12.0)).top(px(9.0)).child(icon(
            CHEVRON_DOWN,
            hsla(colors.muted_foreground, 1.0),
            16.0,
        )));
    let mut root = div().relative().w_full().child(button);
    if snapshot.open {
        let items = snapshot.options.iter().enumerate().map(|(index, option)| {
            div()
                .w_full()
                .px(px(10.0))
                .py(px(6.0))
                .bg(if index == snapshot.selected {
                    hsla(colors.accent, 1.0)
                } else {
                    hsla(colors.background, 1.0)
                })
                .text_color(hsla(colors.foreground, 1.0))
                .child(option.clone())
        });
        root = root.child(
            div()
                .absolute()
                .top(px(38.0))
                .left(px(0.0))
                .w_full()
                .border(px(1.0))
                .border_color(hsla(colors.input, 1.0))
                .children(items),
        );
    }
    root
}
