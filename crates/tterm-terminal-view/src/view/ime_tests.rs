//! IME tests drive GPUI's public input-handler interface in a test window.

use super::input_tests::Harness;
use gpui_kit::{AppContext, Bounds, EntityInputHandler, TestAppContext, point, px, size};

#[gpui_kit::test]
fn preedit_is_visible_without_writing_and_commit_writes_utf8_once(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[3;5H");
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "拼音😀", Some(2..4), window, cx)
    });
    h.draw(cx);
    h.update(cx, |view, window, cx| {
        assert_eq!(view.marked_text_range(window, cx), Some(0..4));
        assert_eq!(
            view.selected_text_range(false, window, cx).unwrap().range,
            0..0
        );
        assert_eq!(
            view.text_for_range(0..100, &mut None, window, cx),
            Some(String::new())
        );
        let paint = view.preedit.as_ref().expect("prepared visible preedit");
        assert_eq!(paint.line.text.as_ref(), "拼音😀");
        assert!(paint.line.width() > px(0.));
    });
    assert!(h.bytes.borrow().is_empty());
    h.update(cx, |view, window, cx| {
        view.replace_text_in_range(None, "中文", window, cx)
    });
    h.draw(cx);
    assert_eq!(&*h.bytes.borrow(), "中文".as_bytes());
    assert!(h.view.read_with(cx, |view, _| view.preedit.is_none()));
}

#[gpui_kit::test]
fn composition_anchor_ignores_output_cursor_moves(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[3;5H");
    let dummy = Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.)));
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx)
    });
    h.draw(cx);
    let before = h.update(cx, |view, window, cx| {
        view.bounds_for_range(0..0, dummy, window, cx).unwrap()
    });
    h.output(cx, b"\x1b[10;30Hnew output");
    let after = h.update(cx, |view, window, cx| {
        view.bounds_for_range(0..0, dummy, window, cx).unwrap()
    });
    assert_eq!(before, after);
    let offset = h.update(cx, |view, window, cx| {
        view.bounds_for_range(1..1, dummy, window, cx).unwrap()
    });
    assert!(offset.origin.x > before.origin.x);
    assert!(after.size.width > px(0.) && after.size.height > px(0.));
}

#[gpui_kit::test]
fn composing_keys_and_confirmation_enter_never_reach_the_shell(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "ni", None, window, cx)
    });
    cx.simulate_keystrokes(h.window, "enter escape up");
    assert!(h.bytes.borrow().is_empty());
    h.update(cx, |view, window, cx| {
        view.replace_text_in_range(None, "你", window, cx)
    });
    h.event(
        cx,
        gpui_kit::KeyDownEvent {
            keystroke: gpui_kit::Keystroke::parse("enter").unwrap(),
            is_held: false,
            prefer_character_input: true,
        },
    );
    assert_eq!(&*h.bytes.borrow(), "你".as_bytes());
}

#[gpui_kit::test]
fn blur_and_hidden_views_clear_preedit(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "ni", None, window, cx);
        view.focus_changed(false, window, cx);
    });
    h.draw(cx);
    assert!(h.view.read_with(cx, |view, _| view.preedit.is_none()));
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "hao", None, window, cx);
        view.set_visible(false, cx);
    });
    assert!(h.view.read_with(cx, |view, _| !view.composition.active()));
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn hidden_cursor_uses_last_visible_cell_only_during_transient_hide(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[3;5H");
    let dummy = Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.)));
    let visible = h.update(cx, |view, window, cx| {
        view.bounds_for_range(0..0, dummy, window, cx).unwrap()
    });
    h.output(cx, b"\x1b[?25l\x1b[10;20H");
    let transient = h.update(cx, |view, window, cx| {
        view.bounds_for_range(0..0, dummy, window, cx).unwrap()
    });
    assert_eq!(visible, transient);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(600));
    h.draw(cx);
    let hidden = h.update(cx, |view, window, cx| {
        view.bounds_for_range(0..0, dummy, window, cx).unwrap()
    });
    assert!(hidden.origin.y > visible.origin.y);
}

#[gpui_kit::test]
fn commit_is_not_bracketed_and_the_next_enter_still_executes(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?2004h");
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "ni", None, window, cx);
        view.on_key_down(
            &gpui_kit::KeyDownEvent {
                keystroke: gpui_kit::Keystroke::parse("enter").unwrap(),
                is_held: false,
                prefer_character_input: false,
            },
            window,
            cx,
        );
        view.replace_text_in_range(None, "你", window, cx);
    });
    h.event(
        cx,
        gpui_kit::KeyDownEvent {
            keystroke: gpui_kit::Keystroke::parse("enter").unwrap(),
            is_held: false,
            prefer_character_input: false,
        },
    );
    h.event(
        cx,
        gpui_kit::KeyUpEvent {
            keystroke: gpui_kit::Keystroke::parse("enter").unwrap(),
        },
    );
    assert_eq!(&*h.bytes.borrow(), "你".as_bytes());
    cx.simulate_keystrokes(h.window, "enter");
    assert_eq!(&*h.bytes.borrow(), "你\r".as_bytes());
}

#[gpui_kit::test]
fn cancel_and_reset_remove_preedit_without_committing(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "ni", None, window, cx);
        view.unmark_text(window, cx);
        assert!(view.marked_text_range(window, cx).is_none());
        view.replace_and_mark_text_in_range(None, "hao", None, window, cx);
        view.clear(cx);
    });
    h.draw(cx);
    assert!(h.view.read_with(cx, |view, _| view.preedit.is_none()));
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn preedit_bounds_follow_grid_origin_scroll_and_font_metrics(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[3;5H");
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "ni", None, window, cx)
    });
    h.draw(cx);
    let original = h
        .view
        .read_with(cx, |view, _| view.preedit.as_ref().unwrap().bounds);
    h.update(cx, |view, _, cx| {
        view.terminal
            .scroll(tterm_terminal::terminal::Scroll::Delta(1));
        let mut font = view.font.clone();
        font.size *= 2.;
        view.set_font(font, cx);
    });
    h.draw(cx);
    let changed = h
        .view
        .read_with(cx, |view, _| view.preedit.as_ref().unwrap().bounds);
    assert!(changed.size.height > original.size.height);
    assert!(changed.size.width > original.size.width);
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn mouse_candidate_commit_does_not_consume_the_next_enter(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "ni", None, window, cx);
        view.replace_text_in_range(None, "你", window, cx);
    });
    cx.simulate_keystrokes(h.window, "enter");
    assert_eq!(&*h.bytes.borrow(), "你\r".as_bytes());
}

#[gpui_kit::test]
fn utf16_candidate_offsets_use_the_shaped_preedit_without_exposing_scrollback(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?1049h\x1b[4;8H\x1b[?25l");
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "😀a", Some(2..2), window, cx)
    });
    h.draw(cx);
    h.update(cx, |view, window, cx| {
        let element = Bounds::new(point(px(100.), px(50.)), size(px(800.), px(600.)));
        let zero = view.bounds_for_range(0..0, element, window, cx).unwrap();
        let after_emoji = view.bounds_for_range(2..2, element, window, cx).unwrap();
        let preedit = view.preedit.as_ref().unwrap();
        assert_eq!(
            after_emoji.origin.x - zero.origin.x,
            preedit.line.x_for_index(4)
        );
        assert_eq!(view.marked_text_range(window, cx), Some(0..3));
        assert_eq!(
            view.selected_text_range(false, window, cx).unwrap().range,
            0..0
        );
        assert_eq!(
            view.text_for_range(0..500, &mut None, window, cx),
            Some(String::new())
        );
    });
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn preedit_paints_background_and_underline_at_the_candidate_anchor(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[3;5H");
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx)
    });
    h.draw(cx);
    cx.update_window(h.window, |_, window, _| {
        let underlines = window.painted_underlines();
        assert_eq!(underlines.len(), 1);
        assert!(underlines[0].bounds.size.width.as_f32() > 0.);
        assert!(underlines[0].thickness.as_f32() > 0.);
        assert!(window.painted_quads().len() >= 2);
    })
    .unwrap();
}

#[gpui_kit::test]
fn candidate_coordinates_include_element_origin_and_resize(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[3;5H");
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "ni", None, window, cx)
    });
    let bounds = Bounds::new(point(px(70.), px(40.)), size(px(400.), px(200.)));
    h.update(cx, |view, window, cx| {
        view.prepare(bounds, window, cx);
        let caret = view.bounds_for_range(0..0, bounds, window, cx).unwrap();
        let metrics = view.geometry.unwrap();
        assert_eq!(
            caret.origin,
            point(
                px(70. + 4. * metrics.cell_width),
                px(40. + 2. * metrics.cell_height)
            )
        );
        assert_eq!(
            caret.size,
            size(px(metrics.cell_width), px(metrics.cell_height))
        );
        let smaller = Bounds::new(bounds.origin, size(px(120.), px(80.)));
        view.prepare(smaller, window, cx);
        let resized = view.bounds_for_range(0..0, smaller, window, cx).unwrap();
        assert!(resized.origin.x >= smaller.origin.x && resized.origin.y >= smaller.origin.y);
        assert!(resized.origin.y < smaller.origin.y + smaller.size.height);
    });
}

#[gpui_kit::test]
fn scrolling_history_moves_the_preedit_anchor_with_its_grid_row(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let rows = h
        .view
        .read_with(cx, |view, _| view.grid_size().unwrap().screen_lines);
    h.output(cx, &b"history\r\n".repeat(rows + 5));
    h.output(cx, b"\x1b[2;3H");
    h.update(cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "ni", None, window, cx)
    });
    h.draw(cx);
    let (before, before_origin) = h.view.read_with(cx, |view, _| {
        (
            view.preedit.as_ref().unwrap().bounds,
            view.input_origin.unwrap(),
        )
    });
    h.update(cx, |view, _, cx| {
        view.terminal
            .scroll(tterm_terminal::terminal::Scroll::Delta(1));
        view.invalidate(cx);
    });
    h.draw(cx);
    let (after, height, after_origin) = h.view.read_with(cx, |view, _| {
        (
            view.preedit.as_ref().unwrap().bounds,
            view.geometry.unwrap().cell_height,
            view.input_origin.unwrap(),
        )
    });
    assert_eq!(
        (after.origin.y - after_origin.y) - (before.origin.y - before_origin.y),
        px(height)
    );
    assert!(h.bytes.borrow().is_empty());
}
