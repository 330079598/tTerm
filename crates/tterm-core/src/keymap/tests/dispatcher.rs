use crate::keymap::KeymapActionId as A;
use crate::keymap::chord::{EditableTarget, KeyEventData};
use crate::keymap::config::build_chord_index;
use crate::keymap::config::{default_keymap_config, resolve_effective_keymap};
use crate::keymap::dispatcher::*;
use std::collections::BTreeMap;
fn ev(
    key: &'static str,
    code: Option<&'static str>,
    meta: bool,
    ctrl: bool,
    alt: bool,
    shift: bool,
) -> DispatchEvent<'static> {
    DispatchEvent {
        data: KeyEventData {
            key,
            code,
            meta,
            ctrl,
            alt,
            shift,
            composing: false,
        },
        target: EditableTarget { editable: false },
        prevented: false,
        stopped: false,
    }
}
fn env<'a>(
    index: &'a BTreeMap<String, A>,
    handlers: &'a BTreeMap<A, usize>,
    mac: bool,
) -> DispatchEnvironment<'a> {
    DispatchEnvironment {
        suppressed: false,
        layer_open: false,
        mac,
        index,
        handlers,
    }
}
#[test]
fn dispatch_keydown_event_dispatches_bound_chord_to_handler_and_consumes_event() {
    let i = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let h = BTreeMap::from([(A::TerminalFind, 1)]);
    let e = env(&i, &h, false);
    let mut x = ev("f", Some("KeyF"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), Some(A::TerminalFind));
    assert!(x.prevented && x.stopped);
}
#[test]
fn dispatch_keydown_event_gives_handlers_physical_digit_on_mac_option_symbol() {
    let i = BTreeMap::from([("alt+1".into(), A::TabsSwitchToNth)]);
    let h = BTreeMap::from([(A::TabsSwitchToNth, 1)]);
    let e = env(&i, &h, true);
    let mut x = ev("¡", Some("Digit1"), false, false, true, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), Some(A::TabsSwitchToNth));
    assert_eq!(x.data.code, Some("Digit1"));
}
#[test]
fn dispatch_keydown_event_passes_unbound_chords_through() {
    let i = BTreeMap::new();
    let h = BTreeMap::new();
    let e = env(&i, &h, false);
    let mut x = ev("a", Some("KeyA"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), None);
    assert!(!x.prevented);
}
#[test]
fn dispatch_keydown_event_does_not_consume_bound_without_handler() {
    let i = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let h = BTreeMap::new();
    let e = env(&i, &h, false);
    let mut x = ev("f", Some("KeyF"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), None);
    assert!(!x.prevented);
}
#[test]
fn dispatch_keydown_event_skips_actions_disallowing_editable_targets() {
    let i = BTreeMap::from([("mod+a".into(), A::SftpSelectAll)]);
    let h = BTreeMap::from([(A::SftpSelectAll, 1)]);
    let e = env(&i, &h, false);
    let mut x = ev("a", Some("KeyA"), false, true, false, false);
    x.target.editable = true;
    assert_eq!(dispatch_keydown_event(&mut x, &e), None);
}
#[test]
fn dispatch_keydown_event_allows_action_outside_editable_surface() {
    let i = BTreeMap::from([("mod+a".into(), A::SftpSelectAll)]);
    let h = BTreeMap::from([(A::SftpSelectAll, 1)]);
    let e = env(&i, &h, false);
    let mut x = ev("a", Some("KeyA"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), Some(A::SftpSelectAll));
}
#[test]
fn dispatch_keydown_event_ignores_everything_while_suppressed() {
    let i = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let h = BTreeMap::from([(A::TerminalFind, 1)]);
    let mut e = env(&i, &h, false);
    e.suppressed = true;
    let mut x = ev("f", Some("KeyF"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), None);
}
#[test]
fn dispatch_keydown_event_ignores_everything_while_modal_open() {
    let i = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let h = BTreeMap::from([(A::TerminalFind, 1)]);
    let mut e = env(&i, &h, false);
    e.layer_open = true;
    let mut x = ev("f", Some("KeyF"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), None);
}
#[test]
fn dispatch_keydown_event_never_dispatches_escape() {
    let i = BTreeMap::new();
    let h = BTreeMap::new();
    let e = env(&i, &h, false);
    let mut x = ev("Escape", Some("Escape"), false, false, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), None);
}
#[test]
fn dispatch_keydown_event_prefers_explicit_ctrl_over_mod_on_mac() {
    let i = BTreeMap::from([
        ("ctrl+tab".into(), A::TabsNext),
        ("mod+tab".into(), A::TerminalFind),
    ]);
    let h = BTreeMap::from([(A::TabsNext, 1), (A::TerminalFind, 1)]);
    let e = env(&i, &h, true);
    let mut x = ev("Tab", Some("Tab"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), Some(A::TabsNext));
}
#[test]
fn dispatch_keydown_event_falls_back_from_physical_ctrl_to_mod_on_mac() {
    let i = BTreeMap::from([("mod+=".into(), A::TerminalZoomIn)]);
    let h = BTreeMap::from([(A::TerminalZoomIn, 1)]);
    let e = env(&i, &h, true);
    let mut x = ev("=", Some("Equal"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), Some(A::TerminalZoomIn));
}
#[test]
fn dispatch_keydown_event_leaves_ctrl_letter_to_terminal_on_mac() {
    let i = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let h = BTreeMap::from([(A::TerminalFind, 1)]);
    let e = env(&i, &h, true);
    let mut x = ev("f", Some("KeyF"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), None);
    assert!(!x.prevented);
}
#[test]
fn dispatch_keydown_event_leaves_ctrl_brackets_to_terminal_on_mac() {
    let i = BTreeMap::from([
        ("mod+[".into(), A::WorkspaceFocusPrevPane),
        ("mod+]".into(), A::WorkspaceFocusNextPane),
    ]);
    let h = BTreeMap::from([
        (A::WorkspaceFocusPrevPane, 1),
        (A::WorkspaceFocusNextPane, 1),
    ]);
    let e = env(&i, &h, true);
    for (k, c) in [("[", "BracketLeft"), ("]", "BracketRight")] {
        let mut x = ev(k, Some(c), false, true, false, false);
        assert_eq!(dispatch_keydown_event(&mut x, &e), None, "key={}", k);
        assert!(!x.prevented);
    }
}
#[test]
fn dispatch_keydown_event_leaves_ctrl_backslash_as_sigquit_on_mac() {
    let i = BTreeMap::from([("mod+\\".into(), A::WorkspaceSplitRight)]);
    let h = BTreeMap::from([(A::WorkspaceSplitRight, 1)]);
    let e = env(&i, &h, true);
    let mut x = ev("\\", Some("Backslash"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), None);
    assert!(!x.prevented);
}
#[test]
fn dispatch_keydown_event_matches_explicit_ctrl_on_windows_and_linux() {
    let i = BTreeMap::from([("ctrl+tab".into(), A::TabsNext)]);
    let h = BTreeMap::from([(A::TabsNext, 1)]);
    let e = env(&i, &h, false);
    let mut x = ev("Tab", Some("Tab"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), Some(A::TabsNext));
}
#[test]
fn attach_keymap_dispatcher_wires_capture_listener_and_detaches() {
    let i = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let h = BTreeMap::from([(A::TerminalFind, 1)]);
    let e = env(&i, &h, false);
    let mut x = ev("f", Some("KeyF"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut x, &e), Some(A::TerminalFind));
    let mut y = ev("f", Some("KeyF"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut y, &e), Some(A::TerminalFind));
}
#[test]
fn real_defaults_resolve_command_library_chord() {
    let e = resolve_effective_keymap(&default_keymap_config(), false);
    let i = build_chord_index(&e);
    let h = BTreeMap::from([(A::CommandLibraryOpen, 1)]);
    let env = env(&i, &h, false);
    let mut x = ev("p", Some("KeyP"), false, true, false, true);
    assert_eq!(
        dispatch_keydown_event(&mut x, &env),
        Some(A::CommandLibraryOpen)
    );
}
#[test]
fn real_defaults_dispatch_new_tab_and_sftp_shortcuts() {
    let e = resolve_effective_keymap(&default_keymap_config(), false);
    let i = build_chord_index(&e);
    let h = BTreeMap::from([(A::WorkspaceNewTab, 1), (A::SftpToggle, 1)]);
    let env = env(&i, &h, false);
    let mut a = ev("t", Some("KeyT"), false, true, false, true);
    assert_eq!(
        dispatch_keydown_event(&mut a, &env),
        Some(A::WorkspaceNewTab)
    );
    let mut b = ev("e", Some("KeyE"), false, true, false, true);
    assert_eq!(dispatch_keydown_event(&mut b, &env), Some(A::SftpToggle));
    let mut c = ev("t", Some("KeyT"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut c, &env), None);
}
#[test]
fn real_defaults_keep_explicit_control_distinct_from_command_on_mac() {
    let e = resolve_effective_keymap(&default_keymap_config(), true);
    let i = build_chord_index(&e);
    let h = BTreeMap::from([(A::TabsNext, 1)]);
    let env = env(&i, &h, true);
    let mut a = ev("Tab", Some("Tab"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut a, &env), Some(A::TabsNext));
    let mut b = ev("Tab", Some("Tab"), true, false, false, false);
    assert_eq!(dispatch_keydown_event(&mut b, &env), None);
}
#[test]
fn real_defaults_open_tab_on_cmd_t_mac() {
    let e = resolve_effective_keymap(&default_keymap_config(), true);
    let i = build_chord_index(&e);
    let h = BTreeMap::from([(A::WorkspaceNewTab, 1)]);
    let env = env(&i, &h, true);
    let mut a = ev("t", Some("KeyT"), true, false, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut a, &env),
        Some(A::WorkspaceNewTab)
    );
    let mut b = ev("t", Some("KeyT"), false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut b, &env), None);
}
#[test]
fn real_defaults_let_bound_but_unmounted_actions_pass_through() {
    let e = resolve_effective_keymap(&default_keymap_config(), false);
    let i = build_chord_index(&e);
    let h = BTreeMap::new();
    let env = env(&i, &h, false);
    let mut x = ev("1", Some("Digit1"), false, false, true, false);
    assert_eq!(dispatch_keydown_event(&mut x, &env), None);
    assert!(!x.prevented);
}
