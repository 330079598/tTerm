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
//! See the W0-04 report's deviation section for the source references.

use crate::layout::{BoundsSnapshot, ElementSizing, LayoutSink, StyleSnapshot};
use gpui_kit::{
    Bounds, Div, FontWeight, IntoElement, ParentElement, Pixels, Rgba, SharedString, Styled, div,
    px,
};

/// The family GPUI resolves to the same macOS system UI font CSS calls
/// `-apple-system`; the exported layout canonicalises both to `system-ui`.
pub const SYSTEM_FONT: &str = ".SystemUIFont";

/// Wraps `child` and records its bounds under `id`.
pub fn parity_element(
    sink: &LayoutSink,
    id: &'static str,
    sizing: ElementSizing,
    style: StyleSnapshot,
    child: impl IntoElement,
) -> Div {
    let sink = sink.clone();
    div()
        .on_children_prepainted(move |bounds, _, _| {
            if let Some(bounds) = bounds.first() {
                sink.record_element(id, sizing, to_snapshot(*bounds), style.clone());
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

fn to_snapshot(bounds: Bounds<Pixels>) -> BoundsSnapshot {
    BoundsSnapshot::new(
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    )
}
