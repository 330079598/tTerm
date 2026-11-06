//! Headless window tests. They need no display server, so CI runs them on all
//! three platforms.
//!
//! The first opens the real `Hello` view through the same `gpui_kit::open_window`
//! call the binary uses, dispatches `Increment`, and asserts the visible
//! counter. The second is P0-25's probe of `debug_selector`: GPUI records the
//! laid-out bounds of a marked element, and `TestAppContext::debug_bounds`
//! reads them back, which is what the parity tooling in W0-04 relies on.

use gpui_kit::prelude::*;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, Bounds, Entity, Point, TestAppContext, VisualTestContext, Window,
    WindowBounds, WindowOptions, div, point, px, size, white,
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

/// The layout P0-25's `debug_selector` probe measures: two marked boxes with
/// margins and no text, so the bounds do not depend on font metrics.
struct SelectorProbe;

impl Render for SelectorProbe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(white())
            .child(
                div()
                    .debug_selector(|| "p0-25.box".into())
                    .w(px(100.))
                    .h(px(50.))
                    .ml(px(24.))
                    .mt(px(16.)),
            )
            .child(
                div()
                    .debug_selector(|| "p0-25.second".into())
                    .w(px(40.))
                    .h(px(30.))
                    .ml(px(24.)),
            )
    }
}

/// P0-25 item 9: a `debug_selector` gives the element's laid-out bounds back
/// through `TestAppContext::debug_bounds`, in logical pixels, and an unmarked
/// selector resolves to `None`. Selectors are only recorded in a debug build
/// with the `test-support` feature, which is the configuration this target is
/// built in.
#[gpui_kit::test]
fn debug_selector_records_the_layout_bounds(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let handle = cx.update(|cx| {
        let bounds = Bounds {
            origin: Point::default(),
            size: size(px(400.), px(300.)),
        };
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            cx,
            |_, cx| cx.new(|_| SelectorProbe),
        )
        .expect("open the selector probe window")
    });

    // Bounds are read through `VisualTestContext`, the test-side stand-in for a
    // window; `TestAppContext` itself has no window to read from.
    let mut window_cx = VisualTestContext::from_window(handle.0, cx);
    window_cx.update(|window, cx| window.render_frame(cx));

    assert_eq!(
        window_cx.debug_bounds("p0-25.box"),
        Some(Bounds {
            origin: point(px(24.), px(16.)),
            size: size(px(100.), px(50.)),
        }),
        "the first marked box sits at its margins with its own size"
    );
    assert_eq!(
        window_cx.debug_bounds("p0-25.second"),
        Some(Bounds {
            origin: point(px(24.), px(66.)),
            size: size(px(40.), px(30.)),
        }),
        "the second box starts below the first one"
    );
    assert_eq!(
        window_cx.debug_bounds("p0-25.missing"),
        None,
        "an unmarked selector is not in the map"
    );
}
