//! Headless tests for the Terminal font editor and its preview.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::*;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{IntoElement, Render, TestAppContext};
use tterm_core::config::AppConfig;
use tterm_core::config::terminal_font_file::SavedTerminalFont;
use tterm_core::i18n::language::Language;
use tterm_ui::tokens::palette::DARK;

use crate::config_state::{ConfigState, TerminalFontWriter};
use crate::locale::Locale;

struct Probe {
    setting: Entity<FontSetting>,
}

impl Render for Probe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.setting.update(cx, |setting, cx| setting.section(cx))
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime")
}

fn editor(
    cx: &mut TestAppContext,
    config: AppConfig,
    writer: TerminalFontWriter,
) -> (tokio::runtime::Runtime, Entity<FontSetting>) {
    cx.update(gpui_kit::init);
    cx.executor().allow_parking();
    let runtime = runtime();
    let config =
        cx.new(|_| ConfigState::with_terminal_font_writer(config, runtime.handle(), writer));
    let locale = cx.new(|_| Locale::new(Language::En));
    let setting = cx.new(|cx| FontSetting::new(config, locale, DARK, cx));
    (runtime, setting)
}

fn show(cx: &mut TestAppContext, setting: Entity<FontSetting>) -> &mut gpui_kit::VisualTestContext {
    let (_, visual) = cx.add_window_view(|_, _| Probe { setting });
    visual
}

fn open(visual: &mut gpui_kit::VisualTestContext, setting: &Entity<FontSetting>) {
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| setting.open_page(window, cx));
    });
}

fn save_echo(attempts: Arc<Mutex<Vec<(String, u16)>>>) -> TerminalFontWriter {
    Arc::new(move |family, size| {
        attempts
            .lock()
            .expect("font attempts")
            .push((family.clone(), size));
        Ok(SavedTerminalFont { family, size })
    })
}

#[gpui_kit::test]
fn the_draft_previews_before_save_and_cancel_restores_the_saved_values(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let (_runtime, setting) = editor(
        cx,
        AppConfig {
            font_family: "Menlo, monospace".to_string(),
            font_size: 14,
            ..AppConfig::default()
        },
        save_echo(Arc::clone(&attempts)),
    );
    let visual = show(cx, setting.clone());
    open(visual, &setting);
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.set_draft("\"Fira Code\", monospace", "18", window, cx)
        });
    });
    visual.update(|_, cx| {
        let setting = setting.read(cx);
        assert_eq!(setting.preview_family(cx), "Fira Code");
        assert_eq!(setting.preview_size(cx), 18);
        assert!(!setting.saved());
    });
    assert!(attempts.lock().expect("font attempts").is_empty());

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| setting.cancel(window, cx));
    });
    visual.update(|_, cx| {
        let setting = setting.read(cx);
        assert_eq!(setting.family(cx), "Menlo, monospace");
        assert_eq!(setting.size(cx), "14");
        assert_eq!(setting.preview_size(cx), 14);
    });
    assert!(attempts.lock().expect("font attempts").is_empty());
}

#[gpui_kit::test]
fn saving_publishes_the_values_the_writer_read_back(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let (_runtime, setting) = editor(cx, AppConfig::default(), save_echo(Arc::clone(&attempts)));
    let visual = show(cx, setting.clone());
    open(visual, &setting);
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.set_draft("\"Fira Code\", monospace", "21", window, cx)
        });
    });
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| setting.submit(window, cx));
    });
    wait_until(visual, |cx| setting.read(cx).saved());
    visual.update(|_, cx| {
        let config = setting.read(cx).config.read(cx).config();
        assert_eq!(
            (config.font_family.as_str(), config.font_size),
            ("\"Fira Code\", monospace", 21)
        );
        assert_eq!(
            *attempts.lock().expect("font attempts"),
            vec![("\"Fira Code\", monospace".to_string(), 21)]
        );
        assert!(setting.read(cx).error().is_none());
    });
}

#[gpui_kit::test]
fn a_failed_save_keeps_the_draft_and_retry_commits_it(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let failures = Arc::new(AtomicUsize::new(1));
    let remaining = Arc::clone(&failures);
    let recorded = Arc::clone(&attempts);
    let writer: TerminalFontWriter = Arc::new(move |family, size| {
        recorded
            .lock()
            .expect("font attempts")
            .push((family.clone(), size));
        if remaining.fetch_sub(1, Ordering::SeqCst) > 0 {
            Err("disk is full".to_string())
        } else {
            Ok(SavedTerminalFont { family, size })
        }
    });
    let (_runtime, setting) = editor(cx, AppConfig::default(), writer);
    let visual = show(cx, setting.clone());
    open(visual, &setting);
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.set_draft("Menlo, monospace", "18", window, cx)
        });
    });
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| setting.submit(window, cx));
    });
    wait_until(visual, |cx| {
        !setting.read(cx).is_saving() && setting.read(cx).error().is_some()
    });
    visual.update(|_, cx| {
        let setting = setting.read(cx);
        assert_eq!(setting.family(cx), "Menlo, monospace");
        assert_eq!(setting.size(cx), "18");
        assert_eq!(setting.error(), Some("disk is full"));
        assert_eq!(
            setting.config.read(cx).config().font_family,
            AppConfig::default().font_family
        );
    });

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| setting.submit(window, cx));
    });
    wait_until(visual, |cx| setting.read(cx).saved());
    visual.update(|_, cx| {
        assert_eq!(
            setting.read(cx).config.read(cx).config().font_family,
            "Menlo, monospace"
        );
        assert_eq!(attempts.lock().expect("font attempts").len(), 2);
    });
}

#[gpui_kit::test]
fn system_fonts_are_filtered_by_search_and_nerd_fonts_can_be_isolated(cx: &mut TestAppContext) {
    let (_runtime, setting) = editor(
        cx,
        AppConfig::default(),
        save_echo(Arc::new(Mutex::new(Vec::new()))),
    );
    setting.update(cx, |setting, cx| {
        setting.set_system_fonts(vec![
            "Menlo".to_string(),
            "JetBrains Mono Nerd Font".to_string(),
            "Fira Code Nerd Font".to_string(),
        ]);
        cx.notify();
    });
    let visual = show(cx, setting.clone());
    open(visual, &setting);
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.set_search("fira", window, cx);
        });
    });
    visual.update(|_, cx| {
        assert_eq!(
            setting.read(cx).visible_fonts(cx),
            vec!["Fira Code Nerd Font"]
        );
    });
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.set_search("", window, cx);
            setting.show_nerd_fonts_only = true;
        });
    });
    visual.update(|_, cx| {
        assert_eq!(
            setting.read(cx).visible_fonts(cx),
            vec!["JetBrains Mono Nerd Font", "Fira Code Nerd Font"]
        );
    });
}

#[test]
fn the_custom_size_accepts_only_the_shared_range() {
    assert_eq!(parse_size("14"), Ok(14));
    assert_eq!(parse_size(" 6 "), Ok(6));
    assert_eq!(parse_size("72"), Ok(72));
    assert_eq!(parse_size("5"), Err(keys::FONT_SETTINGS_SIZE_RANGE));
    assert_eq!(parse_size("73"), Err(keys::FONT_SETTINGS_SIZE_RANGE));
    assert_eq!(parse_size(""), Err(keys::FONT_SETTINGS_SIZE_REQUIRED));
    assert_eq!(parse_size("14.5"), Err(keys::FONT_SETTINGS_SIZE_INVALID));
}

#[test]
fn the_spacing_fields_accept_the_react_ranges_and_precision() {
    assert_eq!(parse_line_height("1"), Ok(1.0));
    assert_eq!(parse_line_height("1.236"), Ok(1.24));
    assert_eq!(parse_line_height("2"), Ok(2.0));
    assert_eq!(
        parse_line_height(""),
        Err(keys::FONT_SETTINGS_LINE_HEIGHT_REQUIRED)
    );
    assert_eq!(
        parse_line_height("tall"),
        Err(keys::FONT_SETTINGS_LINE_HEIGHT_INVALID)
    );
    assert_eq!(
        parse_line_height("NaN"),
        Err(keys::FONT_SETTINGS_LINE_HEIGHT_INVALID)
    );
    assert_eq!(
        parse_line_height("0.5"),
        Err(keys::FONT_SETTINGS_LINE_HEIGHT_RANGE)
    );
    assert_eq!(
        parse_line_height("2.01"),
        Err(keys::FONT_SETTINGS_LINE_HEIGHT_RANGE)
    );

    assert_eq!(parse_letter_spacing("-5"), Ok(-5));
    assert_eq!(parse_letter_spacing("0"), Ok(0));
    assert_eq!(parse_letter_spacing("10"), Ok(10));
    assert_eq!(
        parse_letter_spacing(""),
        Err(keys::FONT_SETTINGS_LETTER_SPACING_REQUIRED)
    );
    assert_eq!(
        parse_letter_spacing("1.5"),
        Err(keys::FONT_SETTINGS_LETTER_SPACING_INVALID)
    );
    assert_eq!(
        parse_letter_spacing("-6"),
        Err(keys::FONT_SETTINGS_LETTER_SPACING_RANGE)
    );
    assert_eq!(
        parse_letter_spacing("11"),
        Err(keys::FONT_SETTINGS_LETTER_SPACING_RANGE)
    );
    assert_eq!(
        parse_letter_spacing("1000"),
        Err(keys::FONT_SETTINGS_LETTER_SPACING_RANGE)
    );
}

#[gpui_kit::test]
fn page_values_returns_every_draft_and_focuses_the_invalid_spacing_field(cx: &mut TestAppContext) {
    let (_runtime, setting) = editor(
        cx,
        AppConfig::default(),
        save_echo(Arc::new(Mutex::new(Vec::new()))),
    );
    let visual = show(cx, setting.clone());
    open(visual, &setting);
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.set_draft("  Fira Code, monospace  ", "18", window, cx);
            setting.set_spacing_draft("1.236", "-3", window, cx);
            assert_eq!(
                setting.page_values(window, cx),
                Some(("Fira Code, monospace".to_string(), 18, 1.24, -3))
            );

            setting.set_spacing_draft("2.5", "0", window, cx);
            assert!(setting.page_values(window, cx).is_none());
            assert_eq!(
                setting.error(),
                Some("Line height must be between 1 and 2.")
            );
            assert!(
                setting
                    .line_height_focus(cx)
                    .is_some_and(|focus| focus.is_focused(window))
            );

            setting.set_spacing_draft("1.25", "11", window, cx);
            assert!(setting.page_values(window, cx).is_none());
            assert_eq!(
                setting.error(),
                Some("Letter spacing must be between -5 and 10 pixels.")
            );
            assert!(
                setting
                    .letter_spacing_focus(cx)
                    .is_some_and(|focus| focus.is_focused(window))
            );
        });
    });
}

#[gpui_kit::test]
fn page_save_result_shows_status_and_restores_the_saved_draft(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let (_runtime, setting) = editor(
        cx,
        AppConfig {
            font_family: "Menlo, monospace".to_string(),
            font_size: 14,
            terminal_line_height: 1.2,
            terminal_letter_spacing: 1,
            ..AppConfig::default()
        },
        save_echo(Arc::clone(&attempts)),
    );
    let visual = show(cx, setting.clone());
    open(visual, &setting);
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.set_draft("\"Fira Code\", monospace", "18", window, cx);
            setting.set_spacing_draft("1.8", "4", window, cx);
            setting.page_save_result(Err("disk is full".to_string()), window, cx);
            assert_eq!(setting.error(), Some("disk is full"));
            assert!(!setting.saved());
            assert_eq!(setting.family(cx), "\"Fira Code\", monospace");
            assert_eq!(setting.line_height(cx), "1.8");
        });
        window.render_frame(cx);
    });
    assert!(visual.debug_bounds(ERROR_ID).is_some());
    assert!(visual.debug_bounds(SAVED_ID).is_none());

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.page_save_result(Ok(()), window, cx);
            assert!(setting.error().is_none());
            assert!(setting.saved());
            assert_eq!(setting.family(cx), "Menlo, monospace");
            assert_eq!(setting.size(cx), "14");
            assert_eq!(setting.line_height(cx), "1.2");
            assert_eq!(setting.letter_spacing(cx), "1");
        });
        window.render_frame(cx);
    });
    assert!(visual.debug_bounds(ERROR_ID).is_none());
    assert!(visual.debug_bounds(SAVED_ID).is_some());
    assert!(attempts.lock().expect("font attempts").is_empty());
}

#[gpui_kit::test]
fn enter_emits_the_page_submit_event_instead_of_writing_the_font(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let (_runtime, setting) = editor(cx, AppConfig::default(), save_echo(Arc::clone(&attempts)));
    let visual = show(cx, setting.clone());
    open(visual, &setting);
    let submits = Arc::new(AtomicUsize::new(0));
    let recorded = Arc::clone(&submits);
    visual.update(|_, cx| {
        cx.subscribe(&setting, move |_, _: &FontPageSubmit, _| {
            recorded.fetch_add(1, Ordering::SeqCst);
        })
        .detach();
    });
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| setting.focus_family(window, cx));
        window.render_frame(cx);
    });
    visual.simulate_keystrokes("enter");
    assert_eq!(submits.load(Ordering::SeqCst), 1);
    assert!(attempts.lock().expect("font attempts").is_empty());
}

#[gpui_kit::test]
fn the_preview_follows_line_height_and_letter_spacing(cx: &mut TestAppContext) {
    let (_runtime, setting) = editor(
        cx,
        AppConfig {
            font_family: "Menlo, monospace".to_string(),
            font_size: 20,
            ..AppConfig::default()
        },
        save_echo(Arc::new(Mutex::new(Vec::new()))),
    );
    let visual = show(cx, setting.clone());
    open(visual, &setting);
    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.set_spacing_draft("1", "0", window, cx);
        });
        window.render_frame(cx);
    });
    let compact = visual
        .debug_bounds(PREVIEW_ID)
        .expect("the preview box is rendered")
        .size
        .height
        .as_f32();
    let nerd_height = visual
        .debug_bounds(NERD_PREVIEW_ID)
        .expect("the Nerd Font preview is rendered")
        .size
        .height
        .as_f32();
    assert!(visual.debug_bounds(PREVIEW_TEXT_ID).is_some());
    assert!(visual.debug_bounds(PREVIEW_SPACED_ID).is_none());

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.set_spacing_draft("2", "4", window, cx);
        });
        window.render_frame(cx);
    });
    let tall = visual
        .debug_bounds(PREVIEW_ID)
        .expect("the preview box is rendered")
        .size
        .height
        .as_f32();
    assert!(tall > compact);
    assert!(visual.debug_bounds(PREVIEW_TEXT_ID).is_none());
    assert!(visual.debug_bounds(PREVIEW_SPACED_ID).is_some());
    assert_eq!(
        visual
            .debug_bounds(NERD_PREVIEW_ID)
            .expect("the Nerd Font preview is rendered")
            .size
            .height
            .as_f32(),
        nerd_height,
        "the Nerd Font preview keeps its fixed line height"
    );

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.set_spacing_draft("1", "-5", window, cx);
        });
        window.render_frame(cx);
    });
    assert!(
        visual.debug_bounds(PREVIEW_SPACED_ID).is_some(),
        "negative spacing uses the shaped-glyph canvas too"
    );
}

fn wait_until(
    visual: &mut gpui_kit::VisualTestContext,
    mut condition: impl FnMut(&gpui_kit::App) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if visual.update(|_, cx| condition(cx)) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for the save");
        visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
}
