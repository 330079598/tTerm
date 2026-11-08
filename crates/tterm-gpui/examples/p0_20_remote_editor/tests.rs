use std::time::Instant;

use gpui_kit::component::highlighter::SyntaxHighlighter;
use gpui_kit::component::input::{Redo, Rope, Undo};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{EntityInputHandler as _, TestAppContext, VisualTestContext, point, px};

use super::{Probe, document, fixtures};

#[gpui_kit::test]
fn undo_redo_and_search_replace(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (view, cx) = cx.add_window_view(|window, cx| {
        Probe::new("alpha 中文 alpha".into(), "text".into(), window, cx)
    });
    let editor = view.read_with(cx, |view, _| view.editor.clone());
    VisualTestContext::update(cx, |window, app| {
        editor.update(app, |state, cx| {
            state.replace_all("beta 中文 beta", window, cx)
        });
        window.render_frame(app);
        window.dispatch_action(Box::new(Undo), app);
    });
    cx.read(|cx| assert_eq!(editor.read(cx).text().to_string(), "alpha 中文 alpha"));
    VisualTestContext::update(cx, |window, app| {
        window.dispatch_action(Box::new(Redo), app)
    });
    cx.read(|cx| assert_eq!(editor.read(cx).text().to_string(), "beta 中文 beta"));
    VisualTestContext::update(cx, |window, app| {
        editor.update(app, |state, cx| {
            state.open_search(true, cx);
            state.set_search_query("BETA", true, cx);
            assert_eq!(state.search_session().matcher.matched_ranges().len(), 2);
            assert!(state.replace_current_search_match("one", window, cx));
            assert_eq!(state.replace_all_search_matches("two", window, cx), 1);
            assert_eq!(state.text().to_string(), "one 中文 two");
            state.set_search_query("中文", false, cx);
            assert_eq!(state.replace_all_search_matches("汉字😀", window, cx), 1);
            assert_eq!(state.text().to_string(), "one 汉字😀 two");
        });
    });
}

#[gpui_kit::test]
fn composition_protocol_is_not_a_native_ime_test(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (view, cx) =
        cx.add_window_view(|window, cx| Probe::new(String::new(), "text".into(), window, cx));
    let editor = view.read_with(cx, |view, _| view.editor.clone());
    VisualTestContext::update(cx, |window, app| {
        editor.update(app, |state, cx| {
            state.replace_and_mark_text_in_range(None, "zhong", Some(5..5), window, cx);
            assert!(state.marked_text_range(window, cx).is_some());
            state.replace_text_in_range(None, "中文😀", window, cx);
            state.unmark_text(window, cx);
            assert_eq!(state.text().to_string(), "中文😀");
            assert!(state.marked_text_range(window, cx).is_none());
        });
    });
}

#[test]
fn seven_languages_have_no_highlighter_in_the_pinned_default_features() {
    for language in fixtures::LANGUAGES {
        let text = Rope::from(fixtures::sample(language));
        let highlighter = SyntaxHighlighter::new(language);
        let spans = highlighter.highlight(&text);
        println!("language={language} highlight_spans={}", spans.len());
        assert!(
            spans.is_empty(),
            "Reassess the decision if highlighting becomes available"
        );
    }
}

#[test]
fn local_encoding_newlines_and_conflicts() {
    document::local_check().unwrap();
}

#[gpui_kit::test]
fn editor_normalized_text_preserves_file_format(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let original = [
        vec![0xef, 0xbb, 0xbf],
        "中文😀\r\nsecond\r\n".as_bytes().to_vec(),
    ]
    .concat();
    let document = document::Document::decode(original.clone()).unwrap();
    let (view, cx) = cx
        .add_window_view(|window, cx| Probe::new(document.text.clone(), "text".into(), window, cx));
    let editor = view.read_with(cx, |view, _| view.editor.clone());
    cx.read(|cx| {
        assert_eq!(
            document.encode(&editor.read(cx).text().to_string()),
            original
        )
    });
    VisualTestContext::update(cx, |window, app| {
        editor.update(app, |state, cx| {
            state.replace_all("中文😀\nchanged\n", window, cx);
            let expected = [
                vec![0xef, 0xbb, 0xbf],
                "中文😀\r\nchanged\r\n".as_bytes().to_vec(),
            ]
            .concat();
            assert_eq!(document.encode(&state.text().to_string()), expected);
        });
    });
}

#[gpui_kit::test]
fn fifty_mib_open_and_scroll(cx: &mut TestAppContext) {
    let path = fixtures::large_file().unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 52_428_800);
    let start = Instant::now();
    let document = document::Document::open(&path).unwrap();
    let read_ms = start.elapsed().as_millis();
    cx.update(gpui_kit::init);
    let start = Instant::now();
    let (view, cx) =
        cx.add_window_view(|window, cx| Probe::new(document.text, "text".into(), window, cx));
    let editor = view.read_with(cx, |view, _| view.editor.clone());
    VisualTestContext::update(cx, |window, app| window.render_frame(app));
    let open_ms = start.elapsed().as_millis();
    cx.read(|cx| assert_eq!(editor.read(cx).text().len(), 52_428_800));
    for row in [0, 25_600, 51_199, 0] {
        let start = Instant::now();
        VisualTestContext::update(cx, |window, app| {
            editor.update(app, |state, cx| {
                let line_height = state.line_height().unwrap_or(px(20.));
                state.set_scroll_offset(point(px(0.), -(line_height * row as f32)), cx);
            });
            window.render_frame(app);
            window.render_frame(app);
        });
        cx.read(|cx| {
            let visible = editor.read(cx).visible_row_range().unwrap();
            println!(
                "50MiB row={row} visible={visible:?} layout_ms={}",
                start.elapsed().as_millis()
            );
            assert!(visible.contains(&row), "requested row must be visible");
            assert!(visible.len() < 100, "layout must expose only the viewport");
        });
    }
    println!(
        "50MiB bytes=52428800 lines=51200 read_decode_ms={read_ms} open_first_layout_ms={open_ms}; mock renderer, not GPU performance"
    );
    std::fs::remove_file(path).unwrap();
}
