//! The root layout: a fixed title bar above the content area.

use gpui_kit::{Div, IntoElement, ParentElement, Rgba, Styled, div, px};

/// Puts `titlebar` above `content` and paints the window background. The
/// content gets `min_h(0)` so the terminal can shrink inside the flex column.
pub fn frame(titlebar: impl IntoElement, content: impl IntoElement, background: Rgba) -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(background)
        .child(titlebar)
        .child(div().flex_1().min_h(px(0.0)).child(content))
}
