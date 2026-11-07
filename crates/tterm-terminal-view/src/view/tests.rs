//! Behaviour tests for the drawing element.
//!
//! They use GPUI's deterministic test app, so they need no display server.
//! The important properties are user-visible: a hidden view does not ask for
//! frames, an idle unfocused view schedules none, and the blink timer stops
//! when the window loses focus.

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions, point, px, size,
};
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};

use super::font::{FontSet, TerminalFont};
use super::row::{plan_row, shape_row};
use super::{TerminalView, TerminalViewConfig};

fn terminal() -> Arc<Terminal> {
    Arc::new(Terminal::new(
        Config::default(),
        GridSize::new(80, 24),
        CellSize::new(8, 16),
    ))
}

fn open_window(cx: &mut TestAppContext) -> (gpui_kit::AnyWindowHandle, Entity<TerminalView>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| {
        let bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            cx,
            |_, cx| cx.new(|cx| TerminalView::new(cx, terminal(), TerminalViewConfig::default())),
        )
        .expect("open the terminal test window")
    })
}

#[gpui_kit::test]
fn a_hidden_view_suppresses_output_frames(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let view = cx
        .update(|cx| cx.new(|cx| TerminalView::new(cx, terminal(), TerminalViewConfig::default())));

    view.update(cx, |view, cx| view.set_visible(false, cx));
    for _ in 0..100 {
        view.update(cx, |view, cx| view.notify_output(cx));
    }
    view.read_with(cx, |view, _| {
        let stats = view.stats();
        assert_eq!(stats.output_notifications, 100);
        assert_eq!(stats.suppressed_notifications, 100);
        assert_eq!(stats.redraw_requests, 0);
        assert!(!view.is_visible());
    });

    view.update(cx, |view, cx| view.set_visible(true, cx));
    view.read_with(cx, |view, _| {
        assert_eq!(view.stats().redraw_requests, 1);
        assert!(view.is_visible());
    });
}

#[gpui_kit::test]
fn a_visible_view_requests_one_frame_per_output_batch(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let view = cx
        .update(|cx| cx.new(|cx| TerminalView::new(cx, terminal(), TerminalViewConfig::default())));
    for _ in 0..10 {
        view.update(cx, |view, cx| view.notify_output(cx));
    }
    view.read_with(cx, |view, _| {
        assert_eq!(view.stats().output_notifications, 10);
        assert_eq!(view.stats().redraw_requests, 10);
        assert_eq!(view.stats().suppressed_notifications, 0);
    });
}

#[gpui_kit::test]
fn an_idle_unfocused_window_draws_no_frames(cx: &mut TestAppContext) {
    let (handle, view) = open_window(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.blur(cx);
        window.render_frame(cx);
    })
    .expect("render the first frame");

    let frames_after_render = view.read_with(cx, |view, _| view.stats().frames);
    assert!(frames_after_render >= 1);

    cx.executor().advance_clock(Duration::from_secs(10));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .expect("render after the idle wait");

    // The idle wait itself must not have produced a frame. The explicit
    // render above adds exactly one, which is why this is compared to the
    // count taken before the wait plus one.
    let frames_after_wait = view.read_with(cx, |view, _| view.stats().frames);
    assert_eq!(frames_after_wait, frames_after_render + 1);
}

#[gpui_kit::test]
fn the_blink_timer_stops_when_the_view_is_hidden(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let view = cx
        .update(|cx| cx.new(|cx| TerminalView::new(cx, terminal(), TerminalViewConfig::default())));

    // The window in this test is not active, so the normal prepare path
    // would (correctly) keep the timer off. Start it directly to exercise
    // the timer and the hidden-view cancellation.
    view.update(cx, |view, cx| view.update_blink(true, cx));
    assert!(view.read_with(cx, |view, _| view.is_blinking()));
    let toggles_before = view.read_with(cx, |view, _| view.stats().blink_toggles);
    cx.executor().advance_clock(Duration::from_millis(1200));
    cx.run_until_parked();
    let toggles_after = view.read_with(cx, |view, _| view.stats().blink_toggles);
    assert!(
        toggles_after > toggles_before,
        "the timer toggles the cursor while running"
    );

    view.update(cx, |view, cx| view.set_visible(false, cx));
    assert!(!view.read_with(cx, |view, _| view.is_blinking()));
    let toggles_hidden = view.read_with(cx, |view, _| view.stats().blink_toggles);
    cx.executor().advance_clock(Duration::from_secs(5));
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.stats().blink_toggles),
        toggles_hidden,
        "a hidden view stops the blink timer"
    );
}

#[gpui_kit::test]
fn output_after_resize_prepares_rows_and_hits_the_cache(cx: &mut TestAppContext) {
    let (handle, view) = open_window(cx);

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .expect("render the first frame");
    let first = view.read_with(cx, |view, _| view.stats());
    assert!(
        first.cache_misses > 0,
        "building the first frame shapes rows"
    );

    // A second identical frame hits rows that were already shaped.
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .expect("render the second frame");
    let second = view.read_with(cx, |view, _| view.stats());
    assert!(second.cache_hits > first.cache_hits);
    assert!(second.cache_hit_rate() > first.cache_hit_rate());
}

#[gpui_kit::test]
fn a_palette_change_recolors_rows_that_received_no_output(cx: &mut TestAppContext) {
    let (handle, view) = open_window(cx);
    view.read_with(cx, |view, _| view.terminal.write(b"hello\r\nworld\r\n"));
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .expect("render the text");

    // No output follows, so Alacritty reports every text row as undamaged;
    // the new foreground must still reach them.
    let foreground = tterm_terminal::palette::Rgba::rgb(255, 0, 0);
    view.update(cx, |view, cx| {
        let mut palette = view.palette().clone();
        palette.foreground = foreground;
        view.set_palette(palette, cx);
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .expect("render with the new palette");

    view.read_with(cx, |view, _| {
        let frame = view.last_frame.as_ref().expect("a frame was rendered");
        for row in &frame.rows[..2] {
            assert!(!row.glyphs.is_empty());
            assert!(row.glyphs.iter().all(|glyph| glyph.color == foreground));
        }
    });
}

#[test]
fn shaping_places_each_glyph_on_its_column() {
    let platform = gpui_kit::platform::current_platform(true);
    let text_system = gpui_kit::TextSystem::new(platform.text_system());
    let window_text = gpui_kit::WindowTextSystem::new(Arc::new(text_system));
    let font = TerminalFont::default();
    let fonts = FontSet::new(&font);
    let cells: Vec<tterm_terminal::cell::Cell> = "abc"
        .chars()
        .map(|ch| tterm_terminal::cell::Cell {
            ch,
            foreground: tterm_terminal::color::Color::Named(
                tterm_terminal::color::NamedColor::Foreground,
            ),
            background: tterm_terminal::color::Color::Named(
                tterm_terminal::color::NamedColor::Background,
            ),
            attributes: tterm_terminal::cell::Attributes::default(),
            underline_color: None,
            hyperlink: None,
            zerowidth: None,
        })
        .collect();
    let plan = plan_row(
        &cells,
        &tterm_terminal::palette::TerminalPalette::default(),
        crate::glyphs::CellMetrics::new(8.0, 16.0),
    );
    let layout = shape_row(plan, &window_text, &fonts, 14.0);
    assert_eq!(layout.glyphs.len(), 3);
    assert_eq!(
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.column)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(
        layout
            .glyphs
            .iter()
            .all(|glyph| glyph.offset_x.as_f32().abs() < 0.01)
    );
}

#[test]
fn the_config_defaults_match_the_frontend() {
    let config = TerminalViewConfig::default();
    assert_eq!(config.font.size, 14.0);
    assert_eq!(config.font.line_height, 1.0);
    assert_eq!(config.font.letter_spacing, 0.0);
    assert!(config.input.is_none());
}
