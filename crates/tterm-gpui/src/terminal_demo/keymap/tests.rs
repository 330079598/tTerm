//! End-to-end keyboard tests through the view and its session input sink.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext, InputEvent, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
    TestAppContext,
};
use tterm_core::config::keymap::KeymapConfig;
use tterm_core::keymap::actions::KeymapActionId as A;
use tterm_core::keymap::dispatcher::{DispatchEvent, KeymapHandler};
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::input::keyboard::EncoderConfig;
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::{KeymapRouter, encoder_config};

struct Harness {
    window: AnyWindowHandle,
    view: gpui_kit::Entity<TerminalView>,
    model: Arc<Terminal>,
    router: Rc<KeymapRouter>,
    bytes: Rc<RefCell<Vec<u8>>>,
}

impl Harness {
    fn new(
        cx: &mut TestAppContext,
        bindings: &[(A, &[&str])],
        mac: bool,
        keyboard: EncoderConfig,
    ) -> Self {
        cx.update(gpui_kit::init);
        cx.update(tterm_terminal_view::view::bind_keys);
        let config = KeymapConfig {
            bindings: bindings
                .iter()
                .map(|(action, chords)| {
                    (
                        action.as_str().to_string(),
                        Some(chords.iter().map(|chord| (*chord).to_string()).collect()),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
        };
        let router = Rc::new(KeymapRouter::new(&config, mac));
        let route = Rc::clone(&router);
        let model = Arc::new(Terminal::new(
            Config {
                kitty_keyboard: true,
                ..Config::default()
            },
            GridSize::new(80, 24),
            CellSize::new(8, 16),
        ));
        let bytes = Rc::new(RefCell::new(Vec::new()));
        let output = Rc::clone(&bytes);
        let (window, view) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |_, cx| {
                cx.new(|cx| {
                    TerminalView::new(
                        cx,
                        Arc::clone(&model),
                        TerminalViewConfig {
                            input: Some(Rc::new(move |input| {
                                output.borrow_mut().extend_from_slice(input)
                            })),
                            keyboard,
                            keymap: Some(Rc::new(move |event| route.route(event))),
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
            model,
            router,
            bytes,
        };
        harness.update(cx, |view, window, cx| {
            window.focus(&view.focus_handle(), cx);
        });
        harness.draw(cx);
        harness
    }

    fn update<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(
            &mut TerminalView,
            &mut gpui_kit::Window,
            &mut gpui_kit::Context<TerminalView>,
        ) -> R,
    ) -> R {
        cx.update_window(self.window, |_, window, cx| {
            self.view.update(cx, |view, cx| f(view, window, cx))
        })
        .expect("update terminal")
    }

    fn draw(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
        })
        .expect("draw terminal");
    }

    fn output(&self, cx: &mut TestAppContext, bytes: &[u8]) {
        self.model.write(bytes);
        self.update(cx, |view, _, cx| view.notify_output(cx));
        self.draw(cx);
    }

    fn event(&self, cx: &mut TestAppContext, event: impl InputEvent) -> bool {
        cx.update_window(self.window, |_, window, cx| {
            window
                .dispatch_event(event.to_platform_input(), cx)
                .propagate
        })
        .expect("dispatch terminal event")
    }

    fn key_down(&self, cx: &mut TestAppContext, keystroke: Keystroke, prefer: bool) -> bool {
        self.event(
            cx,
            KeyDownEvent {
                keystroke,
                is_held: false,
                prefer_character_input: prefer,
            },
        )
    }

    fn key_up(&self, cx: &mut TestAppContext, keystroke: Keystroke) -> bool {
        self.event(cx, KeyUpEvent { keystroke })
    }

    fn bytes(&self) -> Vec<u8> {
        self.bytes.borrow().clone()
    }

    fn clear(&self) {
        self.bytes.borrow_mut().clear();
    }
}

fn stroke(source: &'static str) -> Keystroke {
    Keystroke::parse(source).expect("parse keystroke")
}

fn key(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
    Keystroke {
        modifiers,
        key: key.to_string(),
        key_char: key_char.map(str::to_string),
    }
}

fn ctrl() -> Modifiers {
    Modifiers {
        control: true,
        ..Modifiers::default()
    }
}

fn alt() -> Modifiers {
    Modifiers {
        alt: true,
        ..Modifiers::default()
    }
}

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Modifiers::default()
    }
}

fn cmd() -> Modifiers {
    Modifiers {
        platform: true,
        ..Modifiers::default()
    }
}

fn ctrl_alt() -> Modifiers {
    Modifiers {
        control: true,
        alt: true,
        ..Modifiers::default()
    }
}

fn accept(calls: Rc<Cell<usize>>) -> KeymapHandler {
    Box::new(move |_: &mut DispatchEvent<'_>, _| {
        calls.set(calls.get() + 1);
        true
    })
}

fn decline(calls: Rc<Cell<usize>>) -> KeymapHandler {
    Box::new(move |_: &mut DispatchEvent<'_>, _| {
        calls.set(calls.get() + 1);
        false
    })
}

#[gpui_kit::test]
fn accepted_shortcut_consumes_keydown_and_writes_no_bytes(cx: &mut TestAppContext) {
    let h = Harness::new(
        cx,
        &[(A::TerminalFind, &["ctrl+f"])],
        false,
        EncoderConfig::default(),
    );
    let calls = Rc::new(Cell::new(0));
    h.router
        .register(A::TerminalFind, accept(Rc::clone(&calls)));

    let propagate = h.key_down(cx, key("f", None, ctrl()), false);

    assert!(!propagate, "the view stops a consumed keydown");
    assert_eq!(calls.get(), 1, "the mounted handler ran once");
    assert!(h.bytes().is_empty(), "a consumed shortcut writes no bytes");
}

#[gpui_kit::test]
fn default_gpui_arrow_names_route_to_handled_actions(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], false, EncoderConfig::default());
    let up = Rc::new(Cell::new(0));
    let down = Rc::new(Cell::new(0));
    let left = Rc::new(Cell::new(0));
    let right = Rc::new(Cell::new(0));
    h.router
        .register(A::WorkspaceFocusPaneUp, accept(Rc::clone(&up)));
    h.router
        .register(A::WorkspaceFocusPaneDown, accept(Rc::clone(&down)));
    h.router
        .register(A::WorkspaceFocusPaneLeft, accept(Rc::clone(&left)));
    h.router
        .register(A::WorkspaceFocusPaneRight, accept(Rc::clone(&right)));

    for name in ["up", "down", "left", "right"] {
        assert!(
            !h.key_down(cx, key(name, None, ctrl_alt()), false),
            "{name} reached the default application action"
        );
        assert!(
            h.bytes().is_empty(),
            "the handled {name} arrow wrote no terminal bytes"
        );
    }
    assert_eq!(up.get(), 1);
    assert_eq!(down.get(), 1);
    assert_eq!(left.get(), 1);
    assert_eq!(right.get(), 1);
}

#[gpui_kit::test]
fn default_gpui_arrows_without_a_handler_fall_through(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], false, EncoderConfig::default());

    for name in ["up", "down", "left", "right"] {
        h.clear();
        h.key_down(cx, key(name, None, ctrl_alt()), false);
        assert!(
            !h.bytes().is_empty(),
            "an unhandled {name} arrow still reaches the terminal encoder"
        );
    }
}

#[gpui_kit::test]
fn gpui_return_and_escape_aliases_match_core_key_names(cx: &mut TestAppContext) {
    let h = Harness::new(
        cx,
        &[(A::TerminalFind, &["ctrl+enter", "ctrl+escape"])],
        false,
        EncoderConfig::default(),
    );
    let calls = Rc::new(Cell::new(0));
    h.router
        .register(A::TerminalFind, accept(Rc::clone(&calls)));

    for name in ["return", "esc"] {
        assert!(
            !h.key_down(cx, key(name, None, ctrl()), false),
            "the {name} alias reached the core named key"
        );
        assert!(
            h.bytes().is_empty(),
            "the handled {name} alias wrote no terminal bytes"
        );
    }
    assert_eq!(calls.get(), 2);
}

#[gpui_kit::test]
fn unbound_chord_still_reaches_the_terminal_encoder(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], false, EncoderConfig::default());

    h.key_down(cx, key("u", None, ctrl()), false);

    assert_eq!(h.bytes(), b"\x15".to_vec(), "Ctrl+U stays a terminal byte");
}

#[gpui_kit::test]
fn bound_action_without_a_mounted_handler_falls_through(cx: &mut TestAppContext) {
    let h = Harness::new(
        cx,
        &[(A::TerminalFind, &["ctrl+f"])],
        false,
        EncoderConfig::default(),
    );

    h.key_down(cx, key("f", None, ctrl()), false);

    assert_eq!(h.bytes(), b"\x06".to_vec(), "Ctrl+F reaches the encoder");
}

#[gpui_kit::test]
fn declined_handler_falls_through_to_the_encoder(cx: &mut TestAppContext) {
    let h = Harness::new(
        cx,
        &[(A::TerminalFind, &["ctrl+f"])],
        false,
        EncoderConfig::default(),
    );
    let calls = Rc::new(Cell::new(0));
    h.router
        .register(A::TerminalFind, decline(Rc::clone(&calls)));

    h.key_down(cx, key("f", None, ctrl()), false);

    assert_eq!(calls.get(), 1, "the mounted handler was asked");
    assert_eq!(h.bytes(), b"\x06".to_vec(), "a refusal passes the key on");
}

#[gpui_kit::test]
fn failed_action_precondition_falls_through_to_the_encoder(cx: &mut TestAppContext) {
    let h = Harness::new(
        cx,
        &[(A::TerminalFind, &["ctrl+f"])],
        false,
        EncoderConfig::default(),
    );
    let available = Rc::new(Cell::new(false));
    let state = Rc::clone(&available);
    h.router.register(
        A::TerminalFind,
        Box::new(move |_: &mut DispatchEvent<'_>, _| state.get()),
    );

    h.key_down(cx, key("f", None, ctrl()), false);
    assert_eq!(
        h.bytes(),
        b"\x06".to_vec(),
        "a failed precondition passes on"
    );

    h.clear();
    available.set(true);
    h.key_down(cx, key("f", None, ctrl()), false);
    assert!(
        h.bytes().is_empty(),
        "the same key is consumed once available"
    );
}

#[gpui_kit::test]
fn mac_ctrl_letter_falls_through_while_command_shortcut_is_consumed(cx: &mut TestAppContext) {
    let h = Harness::new(
        cx,
        &[(A::TerminalFind, &["mod+f"])],
        true,
        EncoderConfig::macos(false),
    );
    let calls = Rc::new(Cell::new(0));
    h.router
        .register(A::TerminalFind, accept(Rc::clone(&calls)));

    h.key_down(cx, key("f", None, ctrl()), false);
    assert_eq!(
        h.bytes(),
        b"\x06".to_vec(),
        "Ctrl+F belongs to the terminal"
    );

    h.clear();
    h.key_down(cx, key("f", None, cmd()), false);
    assert!(h.bytes().is_empty(), "Cmd+F is the application shortcut");
    assert_eq!(calls.get(), 1, "only the Cmd chord reached the handler");
}

#[gpui_kit::test]
fn option_composes_text_when_mac_option_is_meta_is_false(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], true, EncoderConfig::macos(false));

    h.key_down(cx, key("o", Some("ø"), alt()), false);

    assert_eq!(h.bytes(), "ø".as_bytes().to_vec());

    cx.update(|cx| {
        h.view.update(cx, |view, _| {
            view.set_keyboard_config(EncoderConfig::macos(true))
        })
    });
    h.clear();
    h.key_down(cx, key("o", Some("ø"), alt()), false);
    assert_eq!(h.bytes(), b"\x1bo");
    cx.update(|cx| {
        h.view.update(cx, |view, _| {
            view.set_keyboard_config(EncoderConfig::macos(false))
        })
    });
    h.clear();
    h.key_down(cx, key("o", Some("ø"), alt()), false);
    assert_eq!(h.bytes(), "ø".as_bytes());
}

#[gpui_kit::test]
fn option_is_meta_when_mac_option_is_meta_is_true(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], true, EncoderConfig::macos(true));

    h.key_down(cx, key("o", Some("ø"), alt()), false);

    assert_eq!(h.bytes(), b"\x1bo".to_vec());
}

#[gpui_kit::test]
fn option_on_a_named_key_keeps_alt_semantics(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], true, EncoderConfig::macos(false));

    h.key_down(cx, stroke("alt-up"), false);

    assert_eq!(h.bytes(), b"\x1b[1;3A".to_vec());
}

#[gpui_kit::test]
fn windows_altgr_character_preference_skips_the_keymap(cx: &mut TestAppContext) {
    let h = Harness::new(
        cx,
        &[(A::TerminalFind, &["ctrl+alt+e"])],
        false,
        EncoderConfig::default(),
    );
    let calls = Rc::new(Cell::new(0));
    h.router
        .register(A::TerminalFind, accept(Rc::clone(&calls)));
    let altgr = key("e", Some("€"), ctrl_alt());

    let propagate = h.key_down(cx, altgr.clone(), true);
    assert!(
        propagate,
        "character input stays with the text input handler"
    );
    assert_eq!(calls.get(), 0, "AltGr is never an application shortcut");
    assert!(h.bytes().is_empty());

    h.key_down(cx, altgr, false);
    assert_eq!(
        calls.get(),
        1,
        "the same chord without character preference is a shortcut"
    );
    assert!(h.bytes().is_empty());
}

#[gpui_kit::test]
fn non_us_keys_use_the_logical_key_and_key_char(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], false, EncoderConfig::default());

    h.key_down(cx, key("q", Some("ๆ"), Modifiers::default()), false);
    assert_eq!(h.bytes(), "ๆ".as_bytes().to_vec());

    h.clear();
    h.key_down(cx, key("é", Some("é"), Modifiers::default()), false);
    assert_eq!(h.bytes(), "é".as_bytes().to_vec());
}

#[gpui_kit::test]
fn application_cursor_mode_uses_the_ss3_sequence(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], false, EncoderConfig::default());
    h.output(cx, b"\x1b[?1h");

    h.key_down(cx, stroke("up"), false);

    assert_eq!(h.bytes(), b"\x1bOA".to_vec());
}

#[gpui_kit::test]
fn f1_to_f4_use_the_legacy_sequences(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], false, EncoderConfig::default());

    for (key, expected) in [
        ("f1", b"\x1bOP".as_slice()),
        ("f2", b"\x1bOQ".as_slice()),
        ("f3", b"\x1bOR".as_slice()),
        ("f4", b"\x1bOS".as_slice()),
    ] {
        h.clear();
        h.key_down(cx, stroke(key), false);
        assert_eq!(h.bytes(), expected, "{key}");
    }
}

#[gpui_kit::test]
fn ctrl_c0_bytes_are_unchanged(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], false, EncoderConfig::default());

    for (keystroke, expected) in [
        (key("a", None, ctrl()), b"\x01".as_slice()),
        (key("[", None, ctrl()), b"\x1b".as_slice()),
        (key("space", Some(" "), ctrl()), b"\0".as_slice()),
        (key("?", None, ctrl()), b"\x7f".as_slice()),
    ] {
        h.clear();
        h.key_down(cx, keystroke, false);
        assert_eq!(h.bytes(), expected);
    }
}

#[gpui_kit::test]
fn tab_keys_escape_root_focus_traversal(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], false, EncoderConfig::default());

    h.key_down(cx, key("tab", Some("\t"), Modifiers::default()), false);
    assert_eq!(h.bytes(), b"\t".to_vec());

    h.clear();
    h.key_down(cx, key("tab", Some("\t"), shift()), false);
    assert_eq!(h.bytes(), b"\x1b[Z".to_vec());
}

#[gpui_kit::test]
fn kitty_press_repeat_release_pairing_is_unchanged(cx: &mut TestAppContext) {
    let h = Harness::new(cx, &[], false, EncoderConfig::default());
    h.output(cx, b"\x1b[>10u");

    h.key_down(cx, key("a", Some("a"), Modifiers::default()), false);
    h.event(
        cx,
        KeyDownEvent {
            keystroke: key("a", Some("a"), Modifiers::default()),
            is_held: true,
            prefer_character_input: false,
        },
    );
    h.key_up(cx, key("a", Some("a"), Modifiers::default()));

    assert_eq!(h.bytes(), b"\x1b[97u\x1b[97;1:2u\x1b[97;1:3u".to_vec());
}

#[gpui_kit::test]
fn consumed_shortcut_suppresses_its_kitty_keyup(cx: &mut TestAppContext) {
    let h = Harness::new(
        cx,
        &[(A::TerminalFind, &["ctrl+f"])],
        false,
        EncoderConfig::default(),
    );
    let calls = Rc::new(Cell::new(0));
    h.router
        .register(A::TerminalFind, accept(Rc::clone(&calls)));
    h.output(cx, b"\x1b[>10u");

    h.key_down(cx, key("f", None, ctrl()), false);
    h.key_up(cx, key("f", None, ctrl()));
    assert!(
        h.bytes().is_empty(),
        "neither the shortcut nor its release writes"
    );

    h.key_down(cx, key("a", Some("a"), Modifiers::default()), false);
    h.key_up(cx, key("a", Some("a"), Modifiers::default()));
    assert_eq!(
        h.bytes(),
        b"\x1b[97u\x1b[97;1:3u".to_vec(),
        "an unrelated key still reports both events"
    );
}

#[gpui_kit::test]
fn mac_option_setting_only_changes_the_macos_policy() {
    assert!(!encoder_config(true, false).alt_sends_escape);
    assert!(encoder_config(true, true).alt_sends_escape);
    assert!(encoder_config(false, false).alt_sends_escape);
    assert!(encoder_config(false, true).alt_sends_escape);
}
