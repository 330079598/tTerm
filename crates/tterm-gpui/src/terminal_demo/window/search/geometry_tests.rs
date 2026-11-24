//! The search bar's geometry rules, without a window.
//!
//! The production bar feeds these functions its measured pane and border box.
//! These tests pin the wide, narrow-window and narrow-pane cases and the clamp
//! a split or window resize applies, so a frame ordering bug cannot hide a
//! geometry regression.

use gpui_kit::{Bounds, Pixels, Size, point, px, size};

use super::drag::{
    BAR_DEFAULT_RIGHT, BAR_DEFAULT_TOP, BAR_MARGIN, BAR_WIDTH, BarWidth, NARROW_INSET,
    NARROW_WINDOW, bar_width, inset, place,
};

fn surface(width: f32, height: f32) -> Bounds<Pixels> {
    Bounds {
        origin: point(px(0.), px(0.)),
        size: size(px(width), px(height)),
    }
}

fn bar(width: f32, height: f32) -> Size<Pixels> {
    size(px(width), px(height))
}

#[test]
fn a_wide_pane_keeps_the_measured_522_by_44_bar_at_12_by_14() {
    assert_eq!(bar_width(800., Some(800.)), BarWidth::Fixed(px(BAR_WIDTH)));
    assert_eq!(inset(800.), BAR_MARGIN);
    let (origin, placed_surface, placed_bar) =
        place(800., Some(surface(800., 600.)), Some(bar(522., 44.)), None).expect("geometry");
    assert_eq!(origin, point(px(800. - 14. - 522.), px(12.)));
    assert_eq!(origin, point(px(264.), px(BAR_DEFAULT_TOP)));
    assert_eq!(placed_surface, surface(800., 600.));
    assert_eq!(placed_bar, bar(522., 44.));
    assert_eq!(BAR_DEFAULT_RIGHT, 14.);
}

#[test]
fn a_window_at_most_640_fills_between_ten_pixel_gutters() {
    assert_eq!(bar_width(NARROW_WINDOW, Some(640.)), BarWidth::Fill);
    assert_eq!(bar_width(480., Some(480.)), BarWidth::Fill);
    assert_eq!(inset(640.), NARROW_INSET);

    let (origin, _, _) =
        place(480., Some(surface(480., 360.)), Some(bar(460., 78.)), None).expect("geometry");
    assert_eq!(origin, point(px(10.), px(10.)));
}

#[test]
fn a_window_above_640_keeps_the_fixed_width() {
    assert_eq!(bar_width(641., Some(641.)), BarWidth::Fixed(px(BAR_WIDTH)));
    assert_eq!(inset(641.), BAR_MARGIN);
}

#[test]
fn a_narrow_pane_shrinks_to_its_margins_but_stays_draggable() {
    assert_eq!(bar_width(1200., Some(538.)), BarWidth::Fixed(px(BAR_WIDTH)));
    assert_eq!(bar_width(1200., Some(537.)), BarWidth::Fixed(px(521.)));
    assert_eq!(bar_width(1200., Some(400.)), BarWidth::Fixed(px(384.)));

    // The default origin cannot fit the 14 point right offset, so it clamps to
    // the margin rather than filling and losing the horizontal drag range.
    let (origin, _, _) =
        place(1200., Some(surface(500., 600.)), Some(bar(484., 78.)), None).expect("geometry");
    assert_eq!(origin, point(px(BAR_MARGIN), px(BAR_DEFAULT_TOP)));
}

#[test]
fn dragging_past_every_edge_clamps_to_the_margin() {
    let pane = Some(surface(800., 600.));
    let measured = Some(bar(522., 44.));
    let (top_left, _, _) =
        place(800., pane, measured, Some(point(px(-50.), px(-50.)))).expect("geometry");
    assert_eq!(top_left, point(px(8.), px(8.)));
    let (bottom_right, _, _) =
        place(800., pane, measured, Some(point(px(900.), px(700.)))).expect("geometry");
    assert_eq!(bottom_right, point(px(270.), px(548.)));
}

#[test]
fn dragging_in_a_narrow_window_uses_the_same_eight_pixel_margin() {
    let (origin, _, _) = place(
        480.,
        Some(surface(480., 360.)),
        Some(bar(460., 78.)),
        Some(point(px(900.), px(700.))),
    )
    .expect("geometry");
    assert_eq!(origin, point(px(12.), px(274.)));
}

#[test]
fn a_stationary_bar_is_reclamped_after_a_split_resize() {
    let (origin, _, _) = place(
        1200.,
        Some(surface(400., 300.)),
        Some(bar(384., 78.)),
        Some(point(px(270.), px(548.))),
    )
    .expect("geometry");
    assert_eq!(origin, point(px(8.), px(214.)));
}
