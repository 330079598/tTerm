//! A headless window test: it opens the real `Hello` view through the same
//! `gpui_kit::open_window` call the binary uses, dispatches `Increment`, and
//! asserts the visible counter. It needs no display server, so CI runs it on
//! all three platforms.

use gpui_kit::prelude::*;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, Bounds, Entity, Point, TestAppContext, WindowBounds, WindowOptions, px, size,
};

use crate::app::{Hello, Increment};

fn open_test_window(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Hello>) {
    cx.update(|cx| {
        let bounds = Bounds {
            origin: Point::default(),
            size: size(px(720.), px(480.)),
        };
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            cx,
            |_, cx| cx.new(Hello::new),
        )
        .expect("open the headless test window")
    })
}

#[gpui_kit::test]
fn increment_updates_the_visible_counter(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, view) = open_test_window(cx);

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("clicks").label(), Some("点击次数 / Clicks: 0"));

        // Dispatch the action the way a key binding would: on the focus handle
        // of the view that handles it.
        let focus = view.read(cx).focus.clone();
        focus.dispatch_action(&Increment, window, cx);
        window.render_frame(cx);
        assert_eq!(window.find("clicks").label(), Some("点击次数 / Clicks: 1"));

        // The button runs the same handler, so it counts up too.
        window.click("increment", cx);
        assert_eq!(window.find("clicks").label(), Some("点击次数 / Clicks: 2"));
    })
    .expect("update the headless test window");

    view.read_with(cx, |view, _| assert_eq!(view.clicks, 2));
}
