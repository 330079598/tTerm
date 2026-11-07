//! GPUI event tests assert bytes at the session boundary.

use super::{TerminalView, TerminalViewConfig};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext, Context, Entity, InputEvent, KeyDownEvent, Keystroke,
    TestAppContext, Window,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use tterm_terminal::{
    grid::{CellSize, GridSize},
    terminal::{Config, Terminal},
};

pub(super) struct Harness {
    pub window: AnyWindowHandle,
    pub view: Entity<TerminalView>,
    pub terminal: Arc<Terminal>,
    pub bytes: Rc<RefCell<Vec<u8>>>,
}

impl Harness {
    pub fn new(cx: &mut TestAppContext) -> Self {
        cx.update(gpui_kit::init);
        let terminal = Arc::new(Terminal::new(
            Config {
                kitty_keyboard: true,
                ..Config::default()
            },
            GridSize::new(80, 24),
            CellSize::new(8, 16),
        ));
        let bytes = Rc::new(RefCell::new(Vec::new()));
        let output = bytes.clone();
        let model = terminal.clone();
        let (window, view) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |_, cx| {
                cx.new(|cx| {
                    TerminalView::new(
                        cx,
                        model,
                        TerminalViewConfig {
                            input: Some(Rc::new(move |input| {
                                output.borrow_mut().extend_from_slice(input)
                            })),
                            ..Default::default()
                        },
                    )
                })
            })
            .expect("open terminal test window")
        });
        let harness = Self {
            window,
            view,
            terminal,
            bytes,
        };
        harness.update(cx, |view, window, cx| {
            window.focus(&view.focus_handle, cx);
        });
        harness.draw(cx);
        cx.run_until_parked();
        harness
    }

    pub fn update<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut TerminalView, &mut Window, &mut Context<TerminalView>) -> R,
    ) -> R {
        cx.update_window(self.window, |_, window, cx| {
            self.view.update(cx, |view, cx| f(view, window, cx))
        })
        .expect("update terminal")
    }

    pub fn draw(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
        })
        .expect("draw terminal");
    }

    pub fn output(&self, cx: &mut TestAppContext, bytes: &[u8]) {
        self.terminal.write(bytes);
        self.update(cx, |view, _, cx| view.notify_output(cx));
        self.draw(cx);
    }

    pub fn event(&self, cx: &mut TestAppContext, event: impl InputEvent) -> bool {
        cx.update_window(self.window, |_, window, cx| {
            window
                .dispatch_event(event.to_platform_input(), cx)
                .propagate
        })
        .expect("dispatch terminal event")
    }
}

#[gpui_kit::test]
fn keyboard_events_write_shell_bytes_once(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    cx.simulate_keystrokes(h.window, "a enter ctrl-c up");
    assert_eq!(&*h.bytes.borrow(), b"a\r\x03\x1b[A");
    h.bytes.borrow_mut().clear();
    h.output(cx, b"\x1b[?1h");
    cx.simulate_keystrokes(h.window, "up");
    assert_eq!(&*h.bytes.borrow(), b"\x1bOA");
}

#[gpui_kit::test]
fn character_preference_and_unhandled_shortcuts_propagate(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    assert!(h.event(
        cx,
        KeyDownEvent {
            keystroke: Keystroke::parse("a").unwrap(),
            is_held: false,
            prefer_character_input: true
        }
    ));
    assert!(h.bytes.borrow().is_empty());
    assert!(!h.event(
        cx,
        KeyDownEvent {
            keystroke: Keystroke::parse("enter").unwrap(),
            is_held: false,
            prefer_character_input: false
        }
    ));
    assert_eq!(&*h.bytes.borrow(), b"\r");
}

#[gpui_kit::test]
fn input_restores_the_visible_cursor_phase(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, _, _| {
        view.blink.set_running(true);
        view.blink.toggle();
    });
    cx.simulate_keystrokes(h.window, "x");
    assert!(h.view.read_with(cx, |view, _| view.blink_on()));
    assert_eq!(&*h.bytes.borrow(), b"x");
}

#[gpui_kit::test]
fn repeats_and_key_releases_follow_the_active_protocol(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[>10u");
    h.event(
        cx,
        KeyDownEvent {
            keystroke: Keystroke::parse("a").unwrap(),
            is_held: true,
            prefer_character_input: false,
        },
    );
    h.event(
        cx,
        gpui_kit::KeyUpEvent {
            keystroke: Keystroke::parse("a").unwrap(),
        },
    );
    assert_eq!(&*h.bytes.borrow(), b"\x1b[97;1:2u\x1b[97;1:3u");
}

#[gpui_kit::test]
fn alt_policy_is_configurable_at_the_view_boundary(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, _, _| view.keyboard.alt_sends_escape = true);
    cx.simulate_keystrokes(h.window, "alt-x");
    assert_eq!(&*h.bytes.borrow(), b"\x1bx");
}

#[gpui_kit::test]
fn focus_reports_are_emitted_only_when_requested(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, window, cx| view.focus_changed(true, window, cx));
    assert!(h.bytes.borrow().is_empty());
    h.output(cx, b"\x1b[?1004h");
    h.update(cx, |view, window, cx| view.focus_changed(false, window, cx));
    h.update(cx, |view, window, cx| view.focus_changed(true, window, cx));
    assert_eq!(&*h.bytes.borrow(), b"\x1b[O\x1b[I");
}
