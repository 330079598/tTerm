use crate::keymap::actions::KeymapActionId as A;
use crate::keymap::chord::{Chord, EditableTarget, KeyEventData};
use crate::keymap::config::{build_chord_index, default_keymap_config, resolve_effective_keymap};
use crate::keymap::dispatcher::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

fn event(
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
        target: EditableTarget {
            form_control: false,
            content_editable: false,
            editable_ancestor: false,
        },
        prevented: false,
        stopped: false,
    }
}
fn handlers_for(ids: &[A]) -> BTreeMap<A, Vec<KeymapHandler>> {
    ids.iter()
        .map(|id| {
            (
                *id,
                vec![Box::new(|_: &mut DispatchEvent<'_>, _: &Chord| true) as KeymapHandler],
            )
        })
        .collect()
}
fn runtime<'a>(
    index: &'a BTreeMap<String, A>,
    handlers: &'a mut BTreeMap<A, Vec<KeymapHandler>>,
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
    let index = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let mut handlers = handlers_for(&[A::TerminalFind]);
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("f", None, false, true, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut event, &mut env),
        Some(A::TerminalFind)
    );
    assert!(event.prevented && event.stopped);
}
#[test]
fn dispatch_keydown_event_gives_handlers_physical_digit_on_mac_option_symbol() {
    let index = BTreeMap::from([("alt+1".into(), A::TabsSwitchToNth)]);
    let observed = Rc::new(RefCell::new(None));
    let callback = Rc::clone(&observed);
    let mut handlers = BTreeMap::from([(
        A::TabsSwitchToNth,
        vec![Box::new(move |_: &mut DispatchEvent<'_>, chord: &Chord| {
            *callback.borrow_mut() = Some(chord.clone());
            true
        }) as KeymapHandler],
    )]);
    let mut env = runtime(&index, &mut handlers, true);
    let mut event = event("¡", Some("Digit1"), false, false, true, false);
    assert_eq!(
        dispatch_keydown_event(&mut event, &mut env),
        Some(A::TabsSwitchToNth)
    );
    assert_eq!(
        *observed.borrow(),
        Some(Chord {
            key: "1".into(),
            alt: true,
            ..Default::default()
        })
    );
}
#[test]
fn dispatch_keydown_event_passes_unbound_chords_through() {
    let index = BTreeMap::new();
    let mut handlers = BTreeMap::new();
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("a", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut event, &mut env), None);
    assert!(!event.prevented);
}
#[test]
fn dispatch_keydown_event_does_not_consume_bound_without_handler() {
    let index = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let mut handlers = BTreeMap::new();
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("f", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut event, &mut env), None);
    assert!(!event.prevented);
}
#[test]
fn dispatch_keydown_event_skips_actions_disallowing_editable_targets() {
    let index = BTreeMap::from([("mod+a".into(), A::SftpSelectAll)]);
    let mut handlers = handlers_for(&[A::SftpSelectAll]);
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("a", None, false, true, false, false);
    event.target.form_control = true;
    assert_eq!(dispatch_keydown_event(&mut event, &mut env), None);
}
#[test]
fn dispatch_keydown_event_allows_action_outside_editable_surface() {
    let index = BTreeMap::from([("mod+a".into(), A::SftpSelectAll)]);
    let mut handlers = handlers_for(&[A::SftpSelectAll]);
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("a", None, false, true, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut event, &mut env),
        Some(A::SftpSelectAll)
    );
}
#[test]
fn dispatch_keydown_event_ignores_everything_while_suppressed() {
    let index = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let mut handlers = handlers_for(&[A::TerminalFind]);
    let mut env = runtime(&index, &mut handlers, false);
    env.suppressed = true;
    let mut event = event("f", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut event, &mut env), None);
}
#[test]
fn dispatch_keydown_event_ignores_everything_while_modal_open() {
    let index = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let mut handlers = handlers_for(&[A::TerminalFind]);
    let mut env = runtime(&index, &mut handlers, false);
    env.layer_open = true;
    let mut event = event("f", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut event, &mut env), None);
}
#[test]
fn dispatch_keydown_event_never_dispatches_escape() {
    let index = BTreeMap::new();
    let mut handlers = BTreeMap::new();
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("Escape", None, false, false, false, false);
    assert_eq!(dispatch_keydown_event(&mut event, &mut env), None);
}
#[test]
fn dispatch_keydown_event_prefers_explicit_ctrl_over_mod_on_mac() {
    let index = BTreeMap::from([
        ("ctrl+tab".into(), A::TabsNext),
        ("mod+tab".into(), A::TerminalFind),
    ]);
    let mut handlers = handlers_for(&[A::TabsNext, A::TerminalFind]);
    let mut env = runtime(&index, &mut handlers, true);
    let mut event = event("Tab", None, false, true, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut event, &mut env),
        Some(A::TabsNext)
    );
}
#[test]
fn dispatch_keydown_event_falls_back_from_physical_ctrl_to_mod_on_mac() {
    let index = BTreeMap::from([("mod+=".into(), A::TerminalZoomIn)]);
    let mut handlers = handlers_for(&[A::TerminalZoomIn]);
    let mut env = runtime(&index, &mut handlers, true);
    let mut event = event("=", None, false, true, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut event, &mut env),
        Some(A::TerminalZoomIn)
    );
}
#[test]
fn dispatch_keydown_event_leaves_ctrl_letter_to_terminal_on_mac() {
    let index = BTreeMap::from([
        ("mod+t".into(), A::WorkspaceNewTab),
        ("mod+f".into(), A::TerminalFind),
    ]);
    let mut handlers = handlers_for(&[A::WorkspaceNewTab, A::TerminalFind]);
    let mut env = runtime(&index, &mut handlers, true);
    let mut ctrl_f = event("f", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut ctrl_f, &mut env), None);
    let mut ctrl_t = event("t", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut ctrl_t, &mut env), None);
    let mut cmd_f = event("f", None, true, false, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut cmd_f, &mut env),
        Some(A::TerminalFind)
    );
}
#[test]
fn dispatch_keydown_event_leaves_ctrl_brackets_to_terminal_on_mac() {
    let index = BTreeMap::from([
        ("mod+[".into(), A::WorkspaceFocusPrevPane),
        ("mod+]".into(), A::WorkspaceFocusNextPane),
    ]);
    let mut handlers = handlers_for(&[A::WorkspaceFocusPrevPane, A::WorkspaceFocusNextPane]);
    let mut env = runtime(&index, &mut handlers, true);
    for key in ["[", "]"] {
        let mut ctrl = event(key, None, false, true, false, false);
        assert_eq!(dispatch_keydown_event(&mut ctrl, &mut env), None);
    }
    let mut cmd = event("[", None, true, false, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut cmd, &mut env),
        Some(A::WorkspaceFocusPrevPane)
    );
}
#[test]
fn dispatch_keydown_event_leaves_ctrl_backslash_as_sigquit_on_mac() {
    let index = BTreeMap::from([("mod+\\".into(), A::WorkspaceSplitRight)]);
    let mut handlers = handlers_for(&[A::WorkspaceSplitRight]);
    let mut env = runtime(&index, &mut handlers, true);
    let mut ctrl = event("\\", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut ctrl, &mut env), None);
    let mut cmd = event("\\", None, true, false, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut cmd, &mut env),
        Some(A::WorkspaceSplitRight)
    );
}
#[test]
fn dispatch_keydown_event_matches_explicit_ctrl_on_windows_and_linux() {
    let index = BTreeMap::from([("ctrl+tab".into(), A::TabsNext)]);
    let mut handlers = handlers_for(&[A::TabsNext]);
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("Tab", None, false, true, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut event, &mut env),
        Some(A::TabsNext)
    );
}
#[test]
fn attach_keymap_dispatcher_wires_capture_listener_and_detaches() {
    let index = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let mut handlers = handlers_for(&[A::TerminalFind]);
    let mut env = runtime(&index, &mut handlers, false);
    let mut first = event("f", None, false, true, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut first, &mut env),
        Some(A::TerminalFind)
    );
    env.handlers.clear();
    let mut second = event("f", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut second, &mut env), None);
    assert!(!second.prevented);
}
#[test]
fn real_defaults_resolve_command_library_chord() {
    let effective = resolve_effective_keymap(&default_keymap_config(), false);
    let index = build_chord_index(&effective);
    let mut handlers = handlers_for(&[A::CommandLibraryOpen]);
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("p", None, false, true, false, true);
    assert_eq!(
        dispatch_keydown_event(&mut event, &mut env),
        Some(A::CommandLibraryOpen)
    );
}
#[test]
fn real_defaults_dispatch_new_tab_and_sftp_shortcuts() {
    let effective = resolve_effective_keymap(&default_keymap_config(), false);
    let index = build_chord_index(&effective);
    let mut handlers = handlers_for(&[A::WorkspaceNewTab, A::SftpToggle]);
    let mut env = runtime(&index, &mut handlers, false);
    let mut new_tab = event("t", None, false, true, false, true);
    assert_eq!(
        dispatch_keydown_event(&mut new_tab, &mut env),
        Some(A::WorkspaceNewTab)
    );
    let mut sftp = event("e", None, false, true, false, true);
    assert_eq!(
        dispatch_keydown_event(&mut sftp, &mut env),
        Some(A::SftpToggle)
    );
    let mut shell = event("t", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut shell, &mut env), None);
}
#[test]
fn real_defaults_keep_explicit_control_distinct_from_command_on_mac() {
    let effective = resolve_effective_keymap(&default_keymap_config(), true);
    let index = build_chord_index(&effective);
    let mut handlers = handlers_for(&[A::TabsNext]);
    let mut env = runtime(&index, &mut handlers, true);
    let mut ctrl = event("Tab", None, false, true, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut ctrl, &mut env),
        Some(A::TabsNext)
    );
    let mut cmd = event("Tab", None, true, false, false, false);
    assert_eq!(dispatch_keydown_event(&mut cmd, &mut env), None);
}
#[test]
fn real_defaults_open_tab_on_cmd_t_mac() {
    let effective = resolve_effective_keymap(&default_keymap_config(), true);
    let index = build_chord_index(&effective);
    let mut handlers = handlers_for(&[A::WorkspaceNewTab]);
    let mut env = runtime(&index, &mut handlers, true);
    let mut cmd = event("t", None, true, false, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut cmd, &mut env),
        Some(A::WorkspaceNewTab)
    );
    let mut ctrl = event("t", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut ctrl, &mut env), None);
}
#[test]
fn real_defaults_let_bound_but_unmounted_actions_pass_through() {
    let effective = resolve_effective_keymap(&default_keymap_config(), false);
    let index = build_chord_index(&effective);
    let mut handlers = BTreeMap::new();
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("1", None, false, false, true, false);
    assert_eq!(dispatch_keydown_event(&mut event, &mut env), None);
    assert!(!event.prevented);
}
#[test]
fn dispatch_keydown_event_all_rejecting_handlers_leave_key_unconsumed() {
    let index = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let mut handlers: BTreeMap<A, Vec<KeymapHandler>> = BTreeMap::from([(
        A::TerminalFind,
        vec![Box::new(|_: &mut DispatchEvent<'_>, _: &Chord| false) as KeymapHandler],
    )]);
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("f", None, false, true, false, false);
    assert_eq!(dispatch_keydown_event(&mut event, &mut env), None);
    assert!(!event.prevented && !event.stopped);
}
#[test]
fn dispatch_keydown_event_one_rejecting_and_one_accepting_handler_consumes_key() {
    let index = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let mut handlers: BTreeMap<A, Vec<KeymapHandler>> = BTreeMap::from([(
        A::TerminalFind,
        vec![
            Box::new(|_: &mut DispatchEvent<'_>, _: &Chord| false) as KeymapHandler,
            Box::new(|_: &mut DispatchEvent<'_>, _: &Chord| true) as KeymapHandler,
        ],
    )]);
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("f", None, false, true, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut event, &mut env),
        Some(A::TerminalFind)
    );
    assert!(event.prevented && event.stopped);
}
#[test]
fn dispatch_keydown_event_all_accepting_handlers_consume_key() {
    let index = BTreeMap::from([("mod+f".into(), A::TerminalFind)]);
    let mut handlers = handlers_for(&[A::TerminalFind]);
    let mut env = runtime(&index, &mut handlers, false);
    let mut event = event("f", None, false, true, false, false);
    assert_eq!(
        dispatch_keydown_event(&mut event, &mut env),
        Some(A::TerminalFind)
    );
    assert!(event.prevented && event.stopped);
}
