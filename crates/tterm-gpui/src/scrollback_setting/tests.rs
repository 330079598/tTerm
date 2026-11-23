use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use gpui_kit::TestAppContext;
use tterm_core::config::AppConfig;
use tterm_core::i18n::language::Language;
use tterm_ui::tokens::palette::DARK;

#[gpui_kit::test]
fn editing_keeps_invalid_input_and_returns_focus_on_cancel_or_save(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    // The save finishes on a real Tokio worker, so the test scheduler must be
    // allowed to park for its cross-thread wake.
    cx.executor().allow_parking();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime");
    let config = cx.new(|_| {
        ConfigState::with_writer(AppConfig::default(), runtime.handle(), Arc::new(|_| Ok(())))
    });
    let locale = cx.new(|_| Locale::new(Language::En));
    let editor = cx.new(|cx| ScrollbackSetting::new(config.clone(), locale, DARK, cx));
    let (_, visual) =
        cx.add_window_view(|window, cx| gpui_kit::base::Root::new(editor.clone(), window, cx));
    let previous = visual.update(|window, cx| {
        let previous = cx.focus_handle();
        previous.focus(window, cx);
        editor.update(cx, |editor, cx| editor.open(window, cx));
        previous
    });

    // Typing and Enter go through the production field and key handler. The
    // existing value stays in the draft because the typed tail makes it
    // invalid.
    visual.simulate_input("invalid");
    visual.simulate_keystrokes("enter");
    visual.update(|window, cx| {
        assert!(editor.read(cx).open);
        assert!(editor.read(cx).error.is_some());
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
        assert_eq!(editor.read(cx).value(cx), "10000invalid");
        assert!(editor.read(cx).input_focus(cx).is_focused(window));
    });

    visual.simulate_keystrokes("escape");
    visual.update(|window, cx| {
        assert!(!editor.read(cx).open);
        assert!(previous.is_focused(window));
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
    });

    visual.update(|window, cx| {
        editor.update(cx, |editor, cx| editor.open(window, cx));
    });
    visual.simulate_keystrokes(&format!("{}-a", dialog_modifier()));
    visual.simulate_input("400");
    visual.simulate_keystrokes("enter");
    let deadline = Instant::now() + Duration::from_secs(5);
    while visual.update(|_, cx| editor.read(cx).open) {
        assert!(Instant::now() < deadline, "the saved editor should close");
        visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
    visual.update(|window, cx| {
        assert!(previous.is_focused(window));
        assert_eq!(config.read(cx).scrollback_lines(), 400);
    });
}

fn dialog_modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    }
}
