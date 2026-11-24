//! Composition cancellation through the full workspace's key dispatch.

use gpui_kit::{EntityInputHandler as _, Focusable as _};

use super::*;

#[gpui_kit::test]
fn escape_cancels_preedit_before_the_workspace_closes_search(cx: &mut TestAppContext) {
    with_workspace(cx, AppConfig::default(), |root, visual| {
        press(visual, &default_find());
        visual.update(|window, cx| window.render_frame(cx));
        visual.simulate_input("needle");
        let input = visual.update(|_, cx| {
            active_search(&root, cx)
                .expect("the shortcut created the search bar")
                .read(cx)
                .input()
        });
        for preedit in ["中", "中文"] {
            visual.update(|window, cx| {
                input.update(cx, |input, cx| {
                    let length = preedit.encode_utf16().count();
                    input.replace_and_mark_text_in_range(
                        None,
                        preedit,
                        Some(length..length),
                        window,
                        cx,
                    );
                    assert_eq!(input.marked_text_range(window, cx), Some(6..6 + length));
                    assert_eq!(input.value().as_ref(), format!("needle{preedit}"));
                });
            });
        }

        press(visual, "escape");
        visual.update(|window, cx| {
            assert!(search_is_open(&root, cx));
            input.update(cx, |input, cx| {
                assert_eq!(input.marked_text_range(window, cx), None);
                assert_eq!(input.value().as_ref(), "needle");
                assert!(input.focus_handle(cx).is_focused(window));
            });
        });
        press(visual, "escape");
        visual.update(|window, cx| {
            assert!(!search_is_open(&root, cx));
            let active_view = &root.read(cx).tabs.active().view;
            assert!(active_view.read(cx).focus_handle().is_focused(window));
        });
    });
}
