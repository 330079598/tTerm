//! The GPUI-side equivalent of `data-parity`.
//!
//! A marker is a wrapper around exactly one child. Its `on_children_prepainted`
//! listener receives the child's border box and records it in the screen's
//! [`LayoutSink`]. This is the substitute for `debug_selector`: GPUI records
//! debug bounds only in test/debug builds and only exposes them through
//! `VisualTestContext::debug_bounds`, which a headless capture context does not
//! hand out. The wrapper approach works in any render mode and also gives the
//! text element's own bounds, which the masked SSIM needs.
//!
//! The marked child is styled from the same [`StyleSnapshot`] that is recorded.
//! Padding, border width and radius, font size, weight and line height, and the
//! text colour therefore cannot be drawn from one value and reported from
//! another. Background and border colour are deliberately not applied here: the
//! screen keeps painting those itself, so changing only the paint call is a real
//! divergence that the screenshot measurement catches.
//!
//! See the W0-04 report's deviation section for the source references.

use crate::color::to_gpui;
use crate::layout::{BoundsSnapshot, ElementSizing, LayoutSink, StyleSnapshot};
use gpui_kit::{
    Bounds, Div, FontWeight, ParentElement, Pixels, Rgba, SharedString, Styled, div, px,
};

/// The family GPUI resolves to the same macOS system UI font CSS calls
/// `-apple-system`; the exported layout canonicalises both to `system-ui`.
pub const SYSTEM_FONT: &str = ".SystemUIFont";

/// Applies every style value that pixels cannot prove back to a `Div`.
///
/// The screenshot comparison measures background and border colour from the
/// rendered pixels. The remaining fields are declarations, so the painter and
/// the recorder must read them from the same snapshot; this is that single
/// application point.
pub fn apply_snapshot(mut view: Div, style: &StyleSnapshot) -> Div {
    // `system-ui` is the canonical name both layouts use; GPUI draws it with
    // its system font. Any other family is passed through unchanged.
    let family = if style.font_family == "system-ui" {
        SYSTEM_FONT
    } else {
        style.font_family.as_str()
    };
    view = view
        .pt(px(style.padding.top))
        .pr(px(style.padding.right))
        .pb(px(style.padding.bottom))
        .pl(px(style.padding.left))
        .border(px(style.border_width))
        .rounded(px(style.border_radius))
        .font_family(family)
        .text_size(px(style.font_size))
        .line_height(px(style.line_height))
        .font_weight(FontWeight(style.font_weight as f32))
        .text_color(to_gpui(style.color));
    view
}

/// Wraps a styled element and records its bounds and the same style snapshot.
///
/// `build` receives a `Div` that already carries [`apply_snapshot`]. It should
/// add only layout and colours, which are the values the screenshot proves.
pub fn parity_element(
    sink: &LayoutSink,
    id: &'static str,
    sizing: ElementSizing,
    style: StyleSnapshot,
    build: impl FnOnce(Div) -> Div,
) -> Div {
    let sink = sink.clone();
    let recorded = style.clone();
    let child = build(apply_snapshot(div(), &style));
    div()
        .on_children_prepainted(move |bounds, _, _| {
            if let Some(bounds) = bounds.first() {
                sink.record_element(id, sizing, to_snapshot(*bounds), recorded.clone());
            }
        })
        .child(child)
}

/// A text element plus the rectangle the masked comparison leaves out.
///
/// The wrapper is the recording element; the inner div carries the text style
/// and the string. The returned `Div` can still be positioned or laid out by
/// the caller, though the examples use it as a normal flex child.
pub fn parity_text(
    sink: &LayoutSink,
    key: &'static str,
    text: impl Into<SharedString>,
    font_size: f32,
    line_height: f32,
    weight: FontWeight,
    color: Rgba,
) -> Div {
    let sink = sink.clone();
    let text = text.into();
    div()
        .font_family(SYSTEM_FONT)
        .text_size(px(font_size))
        .line_height(px(line_height))
        .font_weight(weight)
        .text_color(color)
        .on_children_prepainted(move |bounds, _, _| {
            if let Some(bounds) = bounds.first() {
                sink.record_text_region(key, to_snapshot(*bounds));
            }
        })
        .child(text)
}

/// A text child that uses the font values of the marked element's snapshot.
pub fn parity_text_for(
    sink: &LayoutSink,
    key: &'static str,
    text: impl Into<SharedString>,
    style: &StyleSnapshot,
) -> Div {
    parity_text(
        sink,
        key,
        text,
        style.font_size,
        style.line_height,
        FontWeight(style.font_weight as f32),
        to_gpui(style.color),
    )
}

fn to_snapshot(bounds: Bounds<Pixels>) -> BoundsSnapshot {
    BoundsSnapshot::new(
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    )
}
