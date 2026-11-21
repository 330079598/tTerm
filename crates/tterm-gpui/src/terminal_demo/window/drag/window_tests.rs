use gpui_kit::prelude::*;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Context, MouseButton, ScrollHandle, TestAppContext, Window, div, point, px};

use super::advance_scroll;

struct Preview;
impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

struct Strip(ScrollHandle);
impl Render for Strip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if cx.has_active_drag() {
            advance_scroll(&self.0, window);
        }
        div()
            .id("viewport")
            .w(px(300.))
            .h(px(40.))
            .flex()
            .overflow_x_scroll()
            .track_scroll(&self.0)
            .children((0usize..8).map(|index| {
                div()
                    .id(("slot", index))
                    .w(px(120.))
                    .h_full()
                    .flex_shrink_0()
                    .on_drag(index, |_, _, _, cx| cx.new(|_| Preview))
            }))
    }
}

#[gpui_kit::test]
fn stationary_drag_keeps_scrolling_across_frames_and_release_stops_it(cx: &mut TestAppContext) {
    let scroll = ScrollHandle::new();
    let (_, visual) = cx.add_window_view(|_, _| Strip(scroll.clone()));
    visual.update(|window, cx| window.render_frame(cx));
    let start = scroll.bounds_for_item(0).unwrap().center();
    let edge = point(scroll.bounds().right() - px(2.), start.y);
    visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        point(start.x + px(10.), start.y),
        MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_move(edge, MouseButton::Left, Default::default());
    let draw = |visual: &mut gpui_kit::VisualTestContext| {
        let scheduled = visual.update(|window, cx| window.simulate_next_frame(cx));
        visual.update(|window, cx| window.render_frame(cx));
        scheduled
    };
    for _ in 0..3 {
        let before = scroll.offset().x;
        assert!(draw(visual) > 0);
        assert!(scroll.offset().x < before);
    }
    let outside = point(edge.x, scroll.bounds().bottom() + px(10.));
    visual.simulate_mouse_move(outside, MouseButton::Left, Default::default());
    let stopped = scroll.offset();
    draw(visual);
    assert_eq!(scroll.offset(), stopped);
    visual.simulate_mouse_up(edge, MouseButton::Left, Default::default());
    let released = scroll.offset();
    draw(visual);
    assert_eq!(scroll.offset(), released);
}
