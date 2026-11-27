//! The search bar's drag gesture through real pointer events.
//!
//! The harness mounts the production [`SearchBar`] beside a production
//! `TerminalView` over a synthetic model. No shell, SSH session or user
//! configuration is involved. An 800x600 window keeps the expected geometry
//! deterministic.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext as _, Context, Entity, EntityInputHandler as _, IntoElement, MouseButton,
    ParentElement, Point, Render, TestAppContext, VisualTestContext, Window, div, point, px, size,
};
use tterm_core::i18n::language::Language;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::drag::BAR_WIDTH;
use super::*;
use crate::app_window::theme;
use crate::locale::{Locale, LocaleGlobal};

type Bytes = Rc<RefCell<Vec<u8>>>;

struct DragRoot {
    view: Entity<TerminalView>,
    bar: Entity<SearchBar>,
}

impl Render for DragRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .child(self.view.clone())
            .child(self.bar.clone())
    }
}

struct Harness {
    root: Entity<DragRoot>,
    model: Arc<Terminal>,
    bytes: Bytes,
}

fn with_harness(cx: &mut TestAppContext, run: impl FnOnce(&mut Harness, &mut VisualTestContext)) {
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
    for index in 0..80 {
        let line = if index % 6 == 0 {
            format!("\x1b[33mneedle\x1b[0m line-{index:03}\r\n")
        } else {
            format!("output line-{index:03} with a needle\r\n")
        };
        model.write(line.as_bytes());
    }
    model.write("中文搜索\r\n".as_bytes());
    let bytes: Bytes = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&bytes);
    let (root, visual) = cx.add_window_view(|window, cx| {
        let locale = cx.new(|_| Locale::new(Language::En));
        LocaleGlobal::set(cx, locale.clone());
        let view = cx.new(|cx| {
            TerminalView::new(
                cx,
                Arc::clone(&model),
                TerminalViewConfig {
                    input: Some(Rc::new(move |input| {
                        sink.borrow_mut().extend_from_slice(input)
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
                locale,
                theme::default_theme().ui,
            )
        });
        window.focus(&view.read(cx).focus_handle(), cx);
        DragRoot { view, bar }
    });
    visual.simulate_resize(size(px(800.), px(600.)));
    let mut harness = Harness { root, model, bytes };
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

    fn render(&mut self, visual: &mut VisualTestContext) {
        visual.update(|window, cx| window.render_frame(cx));
    }

    fn surface(&self, visual: &mut VisualTestContext) -> gpui_kit::Bounds<gpui_kit::Pixels> {
        visual.update(|_, cx| {
            self.root
                .read(cx)
                .view
                .read(cx)
                .bounds()
                .expect("view bounds")
        })
    }

    fn bar_bounds(&mut self, visual: &mut VisualTestContext) -> gpui_kit::Bounds<gpui_kit::Pixels> {
        visual.debug_bounds(BAR_SELECTOR).expect("bar bounds")
    }

    fn handle_center(&mut self, visual: &mut VisualTestContext) -> Point<gpui_kit::Pixels> {
        visual
            .debug_bounds(HANDLE_SELECTOR)
            .expect("handle bounds")
            .center()
    }

    fn open(&mut self, visual: &mut VisualTestContext) {
        self.update(visual, |bar, window, cx| bar.open(window, cx));
        self.render(visual);
    }

    fn type_text(&mut self, visual: &mut VisualTestContext, text: &str) {
        for character in text.chars() {
            let key = character.to_string();
            let keystroke = gpui_kit::Keystroke {
                modifiers: gpui_kit::Modifiers::default(),
                key: key.clone(),
                key_char: Some(key),
            };
            visual.update(|window, cx| window.dispatch_keystroke(keystroke, cx));
        }
    }

    fn press(&mut self, visual: &mut VisualTestContext, chord: &'static str) {
        let keystroke = gpui_kit::Keystroke::parse(chord).expect("the test chord parses");
        visual.update(|window, cx| window.render_frame(cx));
        visual.update(|window, cx| window.dispatch_keystroke(keystroke, cx));
    }

    fn settle(&mut self, visual: &mut VisualTestContext) {
        visual
            .cx
            .executor()
            .advance_clock(SEARCH_DEBOUNCE + std::time::Duration::from_millis(2));
        visual.run_until_parked();
        self.render(visual);
    }

    fn drag(
        &mut self,
        visual: &mut VisualTestContext,
        from: Point<gpui_kit::Pixels>,
        to: Point<gpui_kit::Pixels>,
    ) {
        visual.simulate_mouse_down(from, MouseButton::Left, Default::default());
        visual.simulate_mouse_move(to, MouseButton::Left, Default::default());
        self.render(visual);
        visual.simulate_mouse_up(to, MouseButton::Left, Default::default());
        self.render(visual);
    }

    fn drag_is_active(&self, visual: &mut VisualTestContext) -> bool {
        visual.update(|_, cx| self.root.read(cx).bar.read(cx).drag.is_some())
    }

    fn input_is_focused(&self, visual: &mut VisualTestContext) -> bool {
        visual.update(|window, cx| {
            let bar = self.root.read(cx).bar.read(cx);
            bar.input.read(cx).focus_handle(cx).is_focused(window)
        })
    }

    fn input(
        &self,
        visual: &mut VisualTestContext,
    ) -> Entity<gpui_kit::component::input::InputState> {
        visual.update(|_, cx| self.root.read(cx).bar.read(cx).input.clone())
    }

    /// Marks text through the platform `InputHandler` hook, as an IME does.
    fn ime_mark(&self, visual: &mut VisualTestContext, text: &str) {
        let input = self.input(visual);
        let length = text.encode_utf16().count();
        visual.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, text, Some(length..length), window, cx)
            })
        });
    }

    /// Commits the active composition through the platform hook.
    fn ime_commit(&self, visual: &mut VisualTestContext, text: &str) {
        let input = self.input(visual);
        visual.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_text_in_range(None, text, window, cx);
                input.unmark_text(window, cx);
            })
        });
    }

    /// Cancels the active composition by replacing it with an empty string.
    fn ime_cancel(&self, visual: &mut VisualTestContext) {
        let input = self.input(visual);
        visual.update(|window, cx| {
            input.update(cx, |input, cx| {
                let range = input.marked_text_range(window, cx);
                input.replace_text_in_range(range, "", window, cx);
                input.unmark_text(window, cx);
            })
        });
    }

    fn marked_range(&self, visual: &mut VisualTestContext) -> Option<std::ops::Range<usize>> {
        let input = self.input(visual);
        visual
            .update(|window, cx| input.update(cx, |input, cx| input.marked_text_range(window, cx)))
    }

    fn input_value(&self, visual: &mut VisualTestContext) -> String {
        let input = self.input(visual);
        visual.update(|_, cx| input.read(cx).value().to_string())
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
}

#[gpui_kit::test]
fn the_initial_bar_sits_at_the_top_right_and_only_its_handle_starts_a_drag(
    cx: &mut TestAppContext,
) {
    with_harness(cx, |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);

        let surface = h.surface(visual);
        let bar = h.bar_bounds(visual);
        assert_eq!(bar.top(), surface.top() + px(12.));
        assert_eq!(bar.right(), surface.right() - px(14.));
        assert_eq!(bar.size.width, px(BAR_WIDTH));

        // A press on the count or the input moves neither the bar nor focus.
        let count = visual.debug_bounds(COUNT_SELECTOR).expect("count bounds");
        h.drag(
            visual,
            count.center(),
            count.center() + point(px(80.), px(80.)),
        );
        assert_eq!(h.bar_bounds(visual), bar);
        assert!(!h.drag_is_active(visual));
        assert!(h.input_is_focused(visual));

        // The right button on the handle is inert too.
        let handle = h.handle_center(visual);
        visual.simulate_mouse_down(handle, MouseButton::Right, Default::default());
        visual.simulate_mouse_move(
            handle + point(px(100.), px(100.)),
            MouseButton::Right,
            Default::default(),
        );
        h.render(visual);
        assert_eq!(h.bar_bounds(visual), bar);
        assert!(!h.drag_is_active(visual));
        visual.simulate_mouse_up(handle, MouseButton::Right, Default::default());
        h.render(visual);

        assert!(
            h.bytes.borrow().is_empty(),
            "the drag never writes to the PTY"
        );
        assert_eq!(
            h.model.selection_text(),
            None,
            "the terminal kept no selection"
        );
    });
}

#[gpui_kit::test]
fn dragging_past_all_four_edges_clamps_to_the_eight_pixel_margin(cx: &mut TestAppContext) {
    with_harness(cx, |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        let surface = h.surface(visual);

        let start = h.handle_center(visual);
        h.drag(
            visual,
            start,
            point(surface.left() - px(300.), surface.top() - px(300.)),
        );
        let top_left = h.bar_bounds(visual);
        assert_eq!(top_left.left(), surface.left() + px(8.));
        assert_eq!(top_left.top(), surface.top() + px(8.));
        assert!(!h.drag_is_active(visual));

        let start = h.handle_center(visual);
        h.drag(
            visual,
            start,
            point(surface.right() + px(300.), surface.bottom() + px(300.)),
        );
        let bottom_right = h.bar_bounds(visual);
        assert_eq!(bottom_right.right(), surface.right() - px(8.));
        assert_eq!(bottom_right.bottom(), surface.bottom() - px(8.));
        assert!(!h.drag_is_active(visual));

        assert!(h.bytes.borrow().is_empty());
        assert_eq!(h.model.selection_text(), None);
    });
}

#[gpui_kit::test]
fn releasing_ends_the_gesture_and_later_moves_do_not_drag_the_bar(cx: &mut TestAppContext) {
    with_harness(cx, |h, visual| {
        h.open(visual);
        let surface = h.surface(visual);
        let start = h.handle_center(visual);
        visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
        visual.simulate_mouse_move(
            start + point(px(30.), px(40.)),
            MouseButton::Left,
            Default::default(),
        );
        h.render(visual);
        assert!(h.drag_is_active(visual));
        visual.simulate_mouse_up(
            start + point(px(30.), px(40.)),
            MouseButton::Left,
            Default::default(),
        );
        h.render(visual);
        assert!(!h.drag_is_active(visual));
        let settled = h.bar_bounds(visual);
        assert!(settled.left() >= surface.left() + px(8.));
        assert!(settled.top() >= surface.top() + px(8.));

        visual.simulate_mouse_move(
            point(surface.center().x, surface.center().y),
            None,
            Default::default(),
        );
        h.render(visual);
        assert_eq!(h.bar_bounds(visual), settled);
    });
}

#[gpui_kit::test]
fn a_resize_clamps_the_bar_and_cancels_an_active_drag(cx: &mut TestAppContext) {
    with_harness(cx, |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        let surface = h.surface(visual);
        let start = h.handle_center(visual);
        h.drag(
            visual,
            start,
            point(surface.right() + px(50.), surface.bottom() + px(50.)),
        );
        assert!(!h.drag_is_active(visual));

        // Begin a second gesture and resize before releasing it.
        let start = h.handle_center(visual);
        visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
        visual.simulate_mouse_move(
            start + point(px(30.), px(30.)),
            MouseButton::Left,
            Default::default(),
        );
        h.render(visual);
        assert!(h.drag_is_active(visual));
        visual.simulate_resize(size(px(480.), px(360.)));
        h.render(visual);
        assert!(!h.drag_is_active(visual), "a resize dropped the gesture");

        let surface = h.surface(visual);
        let bar = h.bar_bounds(visual);
        assert!(bar.left() >= surface.left() + px(8.));
        assert!(bar.top() >= surface.top() + px(8.));
        assert!(bar.right() <= surface.right() - px(8.));
        assert!(bar.bottom() <= surface.bottom() - px(8.));

        visual.simulate_mouse_move(
            point(surface.right(), surface.bottom()),
            None,
            Default::default(),
        );
        h.render(visual);
        assert_eq!(h.bar_bounds(visual), bar, "the released bar stayed put");
    });
}

#[gpui_kit::test]
fn a_narrow_pane_wraps_the_controls_and_keeps_every_one_reachable(cx: &mut TestAppContext) {
    with_harness(cx, |h, visual| {
        visual.simulate_resize(size(px(300.), px(600.)));
        h.open(visual);
        h.render(visual);
        let surface = h.surface(visual);
        let bar = h.bar_bounds(visual);
        assert!(bar.size.width <= surface.size.width * 0.95 + px(0.5));
        assert!(bar.left() >= surface.left() + px(8.));
        assert!(bar.right() <= surface.right() - px(8.) + px(0.5));
        for selector in [
            HANDLE_SELECTOR,
            INPUT_SELECTOR,
            COUNT_SELECTOR,
            CASE_OPTION_SELECTOR,
            WORD_OPTION_SELECTOR,
            REGEX_OPTION_SELECTOR,
            PREVIOUS_SELECTOR,
            NEXT_SELECTOR,
            CLOSE_SELECTOR,
        ] {
            let bounds = visual
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} rendered"));
            assert!(
                bounds.left() >= surface.left(),
                "{selector} stays on screen"
            );
            assert!(
                bounds.right() <= surface.right(),
                "{selector} stays on screen"
            );
        }
    });
}

#[gpui_kit::test]
fn closing_while_dragging_drops_the_gesture_and_reopening_does_not_move_alone(
    cx: &mut TestAppContext,
) {
    with_harness(cx, |h, visual| {
        h.open(visual);
        let surface = h.surface(visual);
        let start = h.handle_center(visual);
        visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
        visual.simulate_mouse_move(
            start + point(px(50.), px(60.)),
            MouseButton::Left,
            Default::default(),
        );
        h.render(visual);
        assert!(h.drag_is_active(visual));
        h.update(visual, |bar, window, cx| bar.close(window, true, cx));
        h.render(visual);
        assert!(!h.drag_is_active(visual));
        visual.simulate_mouse_up(
            start + point(px(50.), px(60.)),
            MouseButton::Left,
            Default::default(),
        );
        h.render(visual);

        h.open(visual);
        let reopened = h.bar_bounds(visual);
        visual.simulate_mouse_move(
            point(surface.center().x, surface.center().y),
            None,
            Default::default(),
        );
        h.render(visual);
        assert_eq!(h.bar_bounds(visual), reopened);
        assert!(!h.drag_is_active(visual));
    });
}

#[gpui_kit::test]
fn marked_text_stays_in_the_input_and_cancel_restores_the_previous_query(cx: &mut TestAppContext) {
    with_harness(cx, |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.settle(visual);
        assert_eq!(h.count(visual), 80);
        assert!(h.bytes.borrow().is_empty());

        h.ime_mark(visual, "拼");
        assert_eq!(h.input_value(visual), "needle拼");
        assert_eq!(h.marked_range(visual), Some(6..7));
        assert!(
            h.bytes.borrow().is_empty(),
            "marked text never reaches the PTY"
        );

        h.ime_cancel(visual);
        h.settle(visual);
        assert_eq!(h.input_value(visual), "needle");
        assert_eq!(h.marked_range(visual), None);
        assert_eq!(h.count(visual), 80, "cancel did not disturb the query");
        assert!(h.bytes.borrow().is_empty());
    });
}

#[gpui_kit::test]
fn committing_cjk_ime_text_searches_and_escape_returns_to_the_terminal(cx: &mut TestAppContext) {
    with_harness(cx, |h, visual| {
        h.open(visual);
        assert!(h.input_is_focused(visual));

        h.ime_mark(visual, "中");
        assert_eq!(h.marked_range(visual), Some(0..1));
        assert!(h.bytes.borrow().is_empty());
        h.ime_commit(visual, "中文");
        h.settle(visual);

        assert_eq!(h.input_value(visual), "中文");
        assert_eq!(h.marked_range(visual), None);
        assert_eq!(h.count(visual), 1, "the committed CJK query found its row");
        assert!(
            h.bytes.borrow().is_empty(),
            "the composition never reached the PTY"
        );

        h.press(visual, "escape");
        h.render(visual);
        assert!(!h.update(visual, |bar, _, _| bar.is_open()));
        h.type_text(visual, "x");
        assert_eq!(
            &*h.bytes.borrow(),
            b"x",
            "Escape restored the terminal input"
        );
    });
}

#[gpui_kit::test]
fn escape_while_marked_does_not_close_the_search(cx: &mut TestAppContext) {
    with_harness(cx, |h, visual| {
        h.open(visual);
        h.type_text(visual, "needle");
        h.ime_mark(visual, "中");
        assert_eq!(h.marked_range(visual), Some(6..7));

        h.press(visual, "escape");
        h.render(visual);
        assert!(
            h.update(visual, |bar, _, _| bar.is_open()),
            "the composition owns Escape"
        );
        assert_eq!(h.marked_range(visual), None);
        assert_eq!(h.input_value(visual), "needle");
        assert!(h.input_is_focused(visual));
        assert!(h.bytes.borrow().is_empty());
        h.settle(visual);
        assert_eq!(h.count(visual), 80);
        h.press(visual, "escape");
        h.render(visual);
        assert!(!h.update(visual, |bar, _, _| bar.is_open()));
    });
}
