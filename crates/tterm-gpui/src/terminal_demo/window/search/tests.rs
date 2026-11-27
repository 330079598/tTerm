//! Production search-bar interactions with real key and mouse events.
//!
//! Every test mounts the production [`SearchBar`] next to a production
//! [`TerminalView`] and a synthetic in-process model. There is no SSH
//! connection, PTY, user configuration or desktop window involved.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext as _, Context, Entity, InputEvent as _, IntoElement, Keystroke, Modifiers,
    MouseButton, MouseDownEvent, MouseUpEvent, ParentElement, Render, TestAppContext,
    VisualTestContext, Window, div,
};
use tterm_core::i18n::keys;
use tterm_core::i18n::language::Language;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::search::{
    SearchCancellation, SearchDirection, SearchOptions, SearchOutcome, SearchQuery, SearchResult,
};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::*;
use crate::app_window::theme;
use crate::locale::{Locale, LocaleGlobal};

type Bytes = Rc<RefCell<Vec<u8>>>;

struct SearchRoot {
    view: Entity<TerminalView>,
    bar: Entity<SearchBar>,
}

impl Render for SearchRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .child(self.view.clone())
            .child(self.bar.clone())
    }
}

struct Harness {
    root: Entity<SearchRoot>,
    model: Arc<Terminal>,
    bytes: Bytes,
    locale: Entity<Locale>,
}

fn with_harness(
    cx: &mut TestAppContext,
    lines: &[&str],
    run: impl FnOnce(&mut Harness, &mut VisualTestContext),
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        tterm_ui::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
    });
    let model = Arc::new(Terminal::new(
        Config {
            scrollback: 2_000,
            ..Config::default()
        },
        GridSize::new(80, 24),
        CellSize::new(8, 16),
    ));
    for line in lines {
        model.write(format!("{line}\r\n").as_bytes());
    }
    let bytes: Bytes = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&bytes);
    let mut locale = None;
    let (root, visual) = cx.add_window_view(|window, cx| {
        let locale_entity = cx.new(|_| Locale::new(Language::En));
        LocaleGlobal::set(cx, locale_entity.clone());
        locale = Some(locale_entity.clone());
        let model_for_view = Arc::clone(&model);
        let output = Rc::clone(&sink);
        let view = cx.new(|cx| {
            TerminalView::new(
                cx,
                model_for_view,
                TerminalViewConfig {
                    input: Some(Rc::new(move |input| {
                        output.borrow_mut().extend_from_slice(input)
                    })),
                    ..Default::default()
                },
            )
        });
        let bar = cx.new(|cx| {
            SearchBar::new(
                cx,
                window,
                Arc::clone(&model),
                view.downgrade(),
                locale_entity,
                theme::default_theme().ui,
            )
        });
        window.focus(&view.read(cx).focus_handle(), cx);
        SearchRoot { view, bar }
    });
    let mut harness = Harness {
        root,
        model,
        bytes,
        locale: locale.expect("the locale was built with the window"),
    };
    run(&mut harness, visual);
}

impl Harness {
    fn update<R>(
        &mut self,
        visual: &mut VisualTestContext,
        f: impl FnOnce(&mut SearchBar, &mut Window, &mut Context<SearchBar>) -> R,
    ) -> R {
        visual.update(|window, cx| {
            self.root.update(cx, |root, cx| {
                root.bar.update(cx, |bar, cx| f(bar, window, cx))
            })
        })
    }

    fn update_view<R>(
        &mut self,
        visual: &mut VisualTestContext,
        f: impl FnOnce(&mut TerminalView, &mut Window, &mut Context<TerminalView>) -> R,
    ) -> R {
        visual.update(|window, cx| {
            self.root.update(cx, |root, cx| {
                root.view.update(cx, |view, cx| f(view, window, cx))
            })
        })
    }

    fn open(&mut self, visual: &mut VisualTestContext) {
        self.update(visual, |bar, window, cx| bar.open(window, cx));
        self.render(visual);
    }

    fn render(&mut self, visual: &mut VisualTestContext) {
        visual.update(|window, cx| window.render_frame(cx));
    }

    fn type_text(&mut self, visual: &mut VisualTestContext, text: &str) {
        for character in text.chars() {
            let key = character.to_string();
            let keystroke = Keystroke {
                modifiers: Modifiers::default(),
                key: key.clone(),
                key_char: Some(key),
            };
            visual.update(|window, cx| window.dispatch_keystroke(keystroke, cx));
        }
    }

    fn press(&mut self, visual: &mut VisualTestContext, chord: &'static str) {
        let keystroke = Keystroke::parse(chord).expect("the test chord parses");
        visual.update(|window, cx| window.dispatch_keystroke(keystroke, cx));
    }

    fn click(&mut self, visual: &mut VisualTestContext, selector: &'static str) {
        let bounds = visual
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("no rendered bounds for {selector}"));
        let position = bounds.center();
        visual.update(|window, cx| {
            window.dispatch_event(
                MouseDownEvent {
                    button: MouseButton::Left,
                    position,
                    click_count: 1,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.dispatch_event(
                MouseUpEvent {
                    button: MouseButton::Left,
                    position,
                    click_count: 1,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
        });
    }

    fn settle(&mut self, visual: &mut VisualTestContext) {
        visual
            .cx
            .executor()
            .advance_clock(SEARCH_DEBOUNCE + Duration::from_millis(2));
        visual.run_until_parked();
        self.render(visual);
    }

    fn settle_output(&mut self, visual: &mut VisualTestContext) {
        visual
            .cx
            .executor()
            .advance_clock(OUTPUT_RESCAN_DEBOUNCE + Duration::from_millis(2));
        visual.run_until_parked();
        self.render(visual);
    }

    fn status(&self, visual: &mut VisualTestContext) -> SearchStatus {
        visual.update(|_, cx| self.root.read(cx).bar.read(cx).status().clone())
    }

    fn count(&self, visual: &mut VisualTestContext) -> usize {
        visual.update(|_, cx| {
            self.root
                .read(cx)
                .bar
                .read(cx)
                .result()
                .map_or(0, |result| result.match_count())
        })
    }

    fn active(&self, visual: &mut VisualTestContext) -> Option<usize> {
        visual.update(|_, cx| self.root.read(cx).bar.read(cx).active_index())
    }

    fn overlay_is_empty(&self, visual: &mut VisualTestContext) -> bool {
        visual.update(|_, cx| self.root.read(cx).view.read(cx).search_overlay().is_empty())
    }

    fn overlay_match_count(&self, visual: &mut VisualTestContext) -> usize {
        visual.update(|_, cx| {
            self.root
                .read(cx)
                .view
                .read(cx)
                .search_overlay()
                .result()
                .map_or(0, |result| result.match_count())
        })
    }

    fn bar_is_open(&self, visual: &mut VisualTestContext) -> bool {
        visual.update(|_, cx| self.root.read(cx).bar.read(cx).is_open())
    }

    fn count_text(&self, visual: &mut VisualTestContext) -> String {
        visual.update(|_, cx| self.root.read(cx).bar.read(cx).count_text(cx))
    }

    fn option(&self, visual: &mut VisualTestContext, which: fn(SearchOptions) -> bool) -> bool {
        visual.update(|_, cx| which(self.root.read(cx).bar.read(cx).options()))
    }

    fn retained_top(&self, visual: &mut VisualTestContext) -> usize {
        visual.update(|_, cx| self.root.read(cx).view.read(cx).search_origin().line.max(0) as usize)
    }

    fn emit(&mut self, visual: &mut VisualTestContext, bytes: &[u8]) {
        self.model.write(bytes);
        self.update_view(visual, |view, _, cx| view.notify_output(cx));
        self.render(visual);
    }
}

fn scan(model: &Terminal, query: &str, options: SearchOptions) -> SearchResult {
    let query = SearchQuery::new(query, options).expect("the test query compiles");
    let cancellation = SearchCancellation::new();
    match model.search_retained(&query, &cancellation) {
        SearchOutcome::Complete(result) => result,
        other => panic!("expected a complete scan, got {other:?}"),
    }
}

#[gpui_kit::test]
fn typing_searches_without_reaching_the_terminal_and_esc_restores_it(cx: &mut TestAppContext) {
    with_harness(cx, &["needle one", "other", "needle two"], |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);

        assert_eq!(h.status(visual), SearchStatus::Ready);
        assert_eq!(h.count(visual), 2);
        assert_eq!(h.active(visual), Some(0));
        assert!(
            h.bytes.borrow().is_empty(),
            "the bar never writes to the PTY"
        );
        assert!(!h.overlay_is_empty(visual));

        h.press(visual, "enter");
        assert_eq!(h.active(visual), Some(1));
        h.press(visual, "shift-enter");
        assert_eq!(h.active(visual), Some(0));
        h.click(visual, PREVIOUS_SELECTOR);
        assert_eq!(
            h.active(visual),
            Some(1),
            "previous wraps to the last match"
        );
        h.click(visual, NEXT_SELECTOR);
        assert_eq!(h.active(visual), Some(0), "next wraps from the last match");

        h.press(visual, "escape");
        assert!(!h.bar_is_open(visual));
        assert!(h.overlay_is_empty(visual));
        h.render(visual);
        assert!(
            visual.debug_bounds(BAR_SELECTOR).is_none(),
            "the closed bar leaves no visible controls"
        );
        h.type_text(visual, "abc");
        assert_eq!(
            &*h.bytes.borrow(),
            b"abc",
            "the terminal gets the keyboard back"
        );
        h.type_text(visual, "needle");
        assert_eq!(&*h.bytes.borrow(), b"abcneedle");
    });
}

#[gpui_kit::test]
fn the_close_button_restores_the_terminal_focus_and_hides_the_bar(cx: &mut TestAppContext) {
    with_harness(cx, &["needle"], |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        assert!(h.bar_is_open(visual));
        assert!(visual.debug_bounds(BAR_SELECTOR).is_some());

        h.click(visual, CLOSE_SELECTOR);
        h.render(visual);
        assert!(!h.bar_is_open(visual));
        assert!(h.overlay_is_empty(visual));
        assert!(visual.debug_bounds(BAR_SELECTOR).is_none());
        h.type_text(visual, "q");
        assert_eq!(&*h.bytes.borrow(), b"q");
    });
}

#[gpui_kit::test]
fn the_case_whole_word_and_regex_options_change_the_count(cx: &mut TestAppContext) {
    with_harness(
        cx,
        &["needle", "Needle", "needles", "_needle", "中文 needle"],
        |h, visual| {
            h.open(visual);
            h.type_text(visual, "needle");
            h.settle(visual);
            assert_eq!(h.count(visual), 5);
            assert!(!h.option(visual, |options| options.case_sensitive));

            h.click(visual, CASE_OPTION_SELECTOR);
            h.settle(visual);
            assert_eq!(h.count(visual), 4, "only the capitalised line drops out");
            assert!(h.option(visual, |options| options.case_sensitive));

            h.click(visual, WORD_OPTION_SELECTOR);
            h.settle(visual);
            assert_eq!(
                h.count(visual),
                2,
                "needles and _needle are no longer whole words"
            );
            assert!(h.option(visual, |options| options.whole_word));

            h.click(visual, REGEX_OPTION_SELECTOR);
            h.settle(visual);
            assert!(h.option(visual, |options| options.regex));
            assert_eq!(
                h.count(visual),
                2,
                "the same literal is still a valid regex"
            );
        },
    );
}

#[gpui_kit::test]
fn an_invalid_regex_and_an_empty_result_clear_the_highlight(cx: &mut TestAppContext) {
    with_harness(cx, &["needle", "other"], |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        assert!(!h.overlay_is_empty(visual));

        h.click(visual, REGEX_OPTION_SELECTOR);
        h.settle(visual);
        h.type_text(visual, "(");
        h.settle(visual);
        assert!(matches!(h.status(visual), SearchStatus::Error(_)));
        assert!(h.overlay_is_empty(visual));
        assert_eq!(
            h.count_text(visual),
            "Invalid regular expression",
            "the bar explains the invalid pattern"
        );

        h.click(visual, REGEX_OPTION_SELECTOR);
        h.settle(visual);
        for _ in 0..2 {
            h.press(visual, "backspace");
        }
        h.type_text(visual, "zzz");
        h.settle(visual);
        assert_eq!(h.status(visual), SearchStatus::Empty);
        assert_eq!(h.count(visual), 0);
        assert_eq!(h.overlay_match_count(visual), 0, "nothing is highlighted");
        assert_eq!(h.count_text(visual), "No results");
    });
}

#[gpui_kit::test]
fn chinese_queries_match_and_the_count_translates(cx: &mut TestAppContext) {
    with_harness(cx, &["中文搜索", "英文 only"], |h, visual| {
        h.open(visual);
        h.type_text(visual, "中文");
        h.settle(visual);
        assert_eq!(h.count(visual), 1);
        assert_eq!(h.count_text(visual), "1 / 1");

        visual.update(|_, cx| {
            h.locale
                .update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
        });
        h.render(visual);
        assert_eq!(h.count_text(visual), "1 / 1");
        let no_results = visual.update(|_, cx| {
            h.locale
                .read(cx)
                .text(keys::TERMINAL_SEARCH_NO_RESULTS)
                .expect("the Chinese no-result text is present")
        });
        assert_eq!(no_results, "无结果");
    });
}

#[gpui_kit::test]
fn output_clears_the_result_and_a_settled_rescan_includes_the_new_rows(cx: &mut TestAppContext) {
    with_harness(cx, &["needle", "other"], |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        assert_eq!(h.count(visual), 1);
        assert!(!h.overlay_is_empty(visual));

        h.emit(visual, b"needle three\r\n");
        assert!(
            h.overlay_is_empty(visual),
            "the stale match disappears immediately"
        );
        assert_eq!(h.status(visual), SearchStatus::Waiting);
        assert_eq!(h.count(visual), 0);

        h.settle_output(visual);
        assert_eq!(h.status(visual), SearchStatus::Ready);
        assert_eq!(h.count(visual), 2, "the rescan covers the new row");
    });
}

#[gpui_kit::test]
fn an_in_place_overwrite_and_a_history_clear_remove_the_result(cx: &mut TestAppContext) {
    with_harness(cx, &["needle", "needle"], |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        assert_eq!(h.count(visual), 2);

        // Move to the first line and overwrite its text without adding rows.
        h.emit(visual, b"\x1b[2A\rZZZZZZ");
        assert!(h.overlay_is_empty(visual));
        assert_eq!(h.status(visual), SearchStatus::Waiting);
        h.settle_output(visual);
        assert_eq!(h.count(visual), 1, "the overwritten match no longer exists");

        // ED 2 clears the screen and ED 3 the scrollback; nothing remains.
        h.emit(visual, b"\x1b[H\x1b[2J\x1b[3J");
        assert!(h.overlay_is_empty(visual));
        h.settle_output(visual);
        assert_eq!(h.status(visual), SearchStatus::Empty);
        assert_eq!(h.count(visual), 0);
        assert_eq!(h.overlay_match_count(visual), 0);
    });
}

#[gpui_kit::test]
fn an_older_completion_and_a_stale_version_are_ignored(cx: &mut TestAppContext) {
    with_harness(cx, &["needle", "needle"], |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        assert_eq!(h.count(visual), 2);

        let old = scan(&h.model, "needle", SearchOptions::default());
        h.model.write(b"needle\r\n");
        let new = scan(&h.model, "needle", SearchOptions::default());
        assert_ne!(old.content_version(), new.content_version());

        let request = h.update(visual, |bar, _, _| bar.request_id());
        h.update(visual, |bar, _, cx| {
            bar.accept_outcome(request, SearchOutcome::Complete(old.clone()), cx)
        });
        h.render(visual);
        assert!(
            h.overlay_is_empty(visual),
            "a completion from an older content version is rejected"
        );

        let retried = h.update(visual, |bar, _, _| bar.request_id());
        assert_ne!(retried, request, "the stale rejection scheduled a rescan");
        h.update(visual, |bar, _, cx| {
            bar.accept_outcome(retried, SearchOutcome::Complete(new.clone()), cx)
        });
        h.render(visual);
        assert_eq!(
            h.count(visual),
            3,
            "the completion for the current version is accepted"
        );

        h.update(visual, |bar, _, cx| {
            bar.accept_outcome(retried.wrapping_sub(1), SearchOutcome::Complete(old), cx)
        });
        h.render(visual);
        assert_eq!(
            h.count(visual),
            3,
            "an out-of-order completion cannot replace the result"
        );
    });
}

#[gpui_kit::test]
fn navigation_refuses_a_result_whose_content_version_changed(cx: &mut TestAppContext) {
    with_harness(cx, &["needle", "needle"], |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        assert_eq!(h.count(visual), 2);

        h.model.write(b"needle\r\n");
        h.update(visual, |bar, _, cx| {
            bar.navigate(SearchDirection::Forward, cx);
        });
        h.render(visual);
        assert_eq!(h.status(visual), SearchStatus::Waiting);
        assert!(h.overlay_is_empty(visual));
    });
}

#[gpui_kit::test]
fn rescheduling_cancels_the_previous_scan(cx: &mut TestAppContext) {
    with_harness(cx, &["needle", "needle"], |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        let first = h
            .update(visual, |bar, _, _| bar.current_cancellation())
            .expect("the settled scan kept its token");
        assert!(!first.is_cancelled());

        h.click(visual, CASE_OPTION_SELECTOR);
        let current = h
            .update(visual, |bar, _, _| bar.current_cancellation())
            .expect("the new scan has a token");
        assert!(first.is_cancelled(), "the superseded request is cancelled");
        assert!(!current.is_cancelled());
    });
}

#[gpui_kit::test]
fn navigation_wraps_and_scrolls_an_off_screen_match_into_view(cx: &mut TestAppContext) {
    let lines: Vec<String> = (0..120).map(|index| format!("needle {index:03}")).collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    with_harness(cx, &lines, |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        let first = h.active(visual).expect("a visible match is active");
        assert!(first > 0, "the viewport starts in the history");

        h.press(visual, "enter");
        assert_eq!(h.active(visual), Some(first + 1));
        h.press(visual, "shift-enter");
        assert_eq!(h.active(visual), Some(first));
        h.press(visual, "shift-enter");
        assert_eq!(h.active(visual), Some(first - 1));
        assert!(
            h.retained_top(visual) > 0,
            "the previous match was revealed above the viewport"
        );

        for _ in 0..first - 1 {
            h.press(visual, "shift-enter");
        }
        assert_eq!(
            h.active(visual),
            Some(0),
            "previous wraps to the first match"
        );
        assert_eq!(
            h.retained_top(visual),
            0,
            "the first match scrolls to the top"
        );
        h.press(visual, "shift-enter");
        assert_eq!(
            h.active(visual),
            Some(119),
            "previous wraps from the first match to the last"
        );
        assert_eq!(h.count_text(visual), "120 / 120");
    });
}

#[gpui_kit::test]
fn the_next_and_previous_actions_use_the_search_direction(cx: &mut TestAppContext) {
    with_harness(cx, &["needle", "needle"], |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        h.update(visual, |bar, _, cx| {
            bar.navigate(SearchDirection::Forward, cx)
        });
        assert_eq!(h.active(visual), Some(1));
        h.update(visual, |bar, _, cx| {
            bar.navigate(SearchDirection::Backward, cx)
        });
        assert_eq!(h.active(visual), Some(0));
    });
}
