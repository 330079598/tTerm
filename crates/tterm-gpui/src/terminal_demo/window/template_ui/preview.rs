//! Miniature drawings of the split shapes in the layout picker.

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{Div, ParentElement, Styled, div, px};
use tterm_core::workspace::pane_templates::LayoutShape;
use tterm_core::workspace::tab_groups::SplitOrientation;
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

const PREVIEW_WIDTH: f32 = 88.0;
const PREVIEW_HEIGHT: f32 = 48.0;
const PREVIEW_GAP: f32 = 2.0;
const PREVIEW_RADIUS: f32 = 2.0;

/// Draws one template at the size every picker card reserves for it.
pub(super) fn layout_preview(shape: &LayoutShape, theme: Theme) -> Div {
    div()
        .w(px(PREVIEW_WIDTH))
        .h(px(PREVIEW_HEIGHT))
        .flex()
        .overflow_hidden()
        .child(node(shape, theme))
}

fn node(shape: &LayoutShape, theme: Theme) -> Div {
    match shape {
        LayoutShape::Pane => div().flex_1().min_w_0().min_h_0().p(px(1.0)).child(
            div()
                .size_full()
                .rounded(px(PREVIEW_RADIUS))
                .border(px(1.0))
                .border_color(hsla(theme.colors.border, 1.0))
                .bg(hsla(theme.colors.input, 1.0)),
        ),
        LayoutShape::Split {
            orientation,
            children,
            sizes,
        } => {
            let horizontal = *orientation == SplitOrientation::Horizontal;
            let resolved = resolve_sizes(sizes, children.len());
            let mut split = div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .flex()
                .gap(px(PREVIEW_GAP))
                .when(!horizontal, |split| split.flex_col());
            for (index, child) in children.iter().enumerate() {
                split = split.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .flex_basis(px(0.0))
                        .flex_grow(resolved.get(index).copied().unwrap_or(0.0) as f32)
                        .child(node(child, theme)),
                );
            }
            split
        }
    }
}

fn resolve_sizes(sizes: &[f64], count: usize) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    let fallback = 1.0 / count as f64;
    let mut resolved = (0..count)
        .map(|index| {
            sizes
                .get(index)
                .copied()
                .filter(|size| size.is_finite() && *size > 0.0)
                .unwrap_or(fallback)
        })
        .collect::<Vec<_>>();
    let total: f64 = resolved.iter().sum();
    if !total.is_finite() || total <= 0.0 {
        resolved.fill(fallback);
    } else if (total - 1.0).abs() > f64::EPSILON {
        for size in &mut resolved {
            *size /= total;
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::resolve_sizes;

    #[test]
    fn preview_sizes_are_normalized_and_keep_equal_shares_as_fallback() {
        assert_eq!(resolve_sizes(&[2.0, 1.0], 2), vec![2.0 / 3.0, 1.0 / 3.0]);
        assert_eq!(resolve_sizes(&[f64::NAN], 1), vec![1.0]);
        assert_eq!(resolve_sizes(&[], 2), vec![0.5, 0.5]);
        assert!(resolve_sizes(&[], 0).is_empty());
    }
}
