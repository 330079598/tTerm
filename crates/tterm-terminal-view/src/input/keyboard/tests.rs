//! Table tests for the keyboard encoder.
//!
//! Each expectation names the Alacritty logic or the specification it comes
//! from. The mode comments use Alacritty's `TermMode` names; the application
//! keypad sequences are xterm's because Alacritty 0.17 only parses the mode.

use gpui_kit::Keystroke;
use gpui_kit::Modifiers;
use tterm_terminal::mode::Modes;

use super::EncoderConfig;
use super::KeyLocation;
use super::KeyPress;
use super::KeyState;
use super::encode;

/// A keystroke with no modifiers and no typed character.
fn stroke(key: &str) -> Keystroke {
    Keystroke {
        modifiers: Modifiers::default(),
        key: key.to_string(),
        key_char: None,
    }
}

/// A keystroke with a typed character and no modifiers.
fn typed(key: &str, key_char: &str) -> Keystroke {
    Keystroke {
        modifiers: Modifiers::default(),
        key: key.to_string(),
        key_char: Some(key_char.to_string()),
    }
}

/// The same with modifiers.
fn modified(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
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

fn alt_shift() -> Modifiers {
    Modifiers {
        alt: true,
        shift: true,
        ..Modifiers::default()
    }
}

/// Encodes a plain press with the given Alt policy.
fn pressed(key: &Keystroke, modes: Modes, alt_sends_escape: bool) -> Option<Vec<u8>> {
    encode(
        &KeyPress::new(key),
        modes,
        EncoderConfig { alt_sends_escape },
    )
}

/// Asserts the encoded bytes, panicking with the case's name on failure.
fn assert_bytes(key: &Keystroke, modes: Modes, alt_sends_escape: bool, expected: &[u8]) {
    assert_eq!(
        pressed(key, modes, alt_sends_escape).as_deref(),
        Some(expected)
    );
}

#[test]
fn named_keys_without_modifiers() {
    let normal = Modes::NONE;
    let cases: &[(&str, &[u8])] = &[
        // Alacritty's try_build_named_normal.
        ("up", b"\x1b[A"),
        ("down", b"\x1b[B"),
        ("right", b"\x1b[C"),
        ("left", b"\x1b[D"),
        ("home", b"\x1b[H"),
        ("end", b"\x1b[F"),
        ("pageup", b"\x1b[5~"),
        ("pagedown", b"\x1b[6~"),
        ("insert", b"\x1b[2~"),
        ("delete", b"\x1b[3~"),
        // Alacritty's default bindings for F1-F4; build_sequence for F5-F12.
        ("f1", b"\x1bOP"),
        ("f2", b"\x1bOQ"),
        ("f3", b"\x1bOR"),
        ("f4", b"\x1bOS"),
        ("f5", b"\x1b[15~"),
        ("f6", b"\x1b[17~"),
        ("f7", b"\x1b[18~"),
        ("f8", b"\x1b[19~"),
        ("f9", b"\x1b[20~"),
        ("f10", b"\x1b[21~"),
        ("f11", b"\x1b[23~"),
        ("f12", b"\x1b[24~"),
        // The text path: GPUI stores `\n` for Enter, the terminal wants `\r`.
        ("enter", b"\r"),
        ("tab", b"\t"),
        ("escape", b"\x1b"),
        ("space", b" "),
        // Alacritty's Backspace binding.
        ("backspace", b"\x7f"),
    ];
    for (key, expected) in cases {
        let keystroke = if *key == "enter" {
            typed("enter", "\n")
        } else if *key == "space" {
            typed("space", " ")
        } else {
            stroke(key)
        };
        assert_bytes(&keystroke, normal, true, expected);
    }
}

#[test]
fn modifier_combinations() {
    let normal = Modes::NONE;
    // A modifier makes build_sequence include the default parameter 1 and
    // `;N`; the values are Alacritty's SequenceModifiers + 1.
    assert_bytes(&modified("up", None, ctrl()), normal, true, b"\x1b[1;5A");
    assert_bytes(&modified("up", None, shift()), normal, true, b"\x1b[1;2A");
    assert_bytes(
        &modified("home", None, alt_shift()),
        normal,
        true,
        b"\x1b[1;4H",
    );
    assert_bytes(&modified("f5", None, ctrl()), normal, true, b"\x1b[15;5~");
    // Shift+Tab is a binding; Shift+Alt+Tab is the second one.
    assert_bytes(
        &modified("tab", Some("\t"), shift()),
        normal,
        true,
        b"\x1b[Z",
    );
    assert_bytes(
        &modified("tab", Some("\t"), alt_shift()),
        normal,
        true,
        b"\x1b\x1b[Z",
    );
    // Alt without Shift keeps the tab and only gets the Meta prefix.
    assert_bytes(&modified("tab", Some("\t"), alt()), normal, true, b"\x1b\t");
    // The Alt+Backspace binding beats the Option-composes policy.
    assert_bytes(
        &modified("backspace", None, alt()),
        normal,
        false,
        b"\x1b\x7f",
    );
    assert_bytes(&modified("backspace", None, shift()), normal, true, b"\x7f");
    // Ctrl+Backspace matches no binding, so Alacritty's text path sends the
    // named key's text, which is `\x08`.
    assert_bytes(&modified("backspace", None, ctrl()), normal, true, b"\x08");
    // Enter with Alt is text with the Meta prefix.
    assert_bytes(
        &modified("enter", Some("\n"), alt()),
        normal,
        true,
        b"\x1b\r",
    );
    assert_bytes(&modified("enter", Some("\n"), alt()), normal, false, b"\r");
}

#[test]
fn application_cursor_mode() {
    let app = Modes::APP_CURSOR;
    // Alacritty's default bindings for +BindingMode::APP_CURSOR.
    assert_bytes(&stroke("up"), app, true, b"\x1bOA");
    assert_bytes(&stroke("down"), app, true, b"\x1bOB");
    assert_bytes(&stroke("right"), app, true, b"\x1bOC");
    assert_bytes(&stroke("left"), app, true, b"\x1bOD");
    assert_bytes(&stroke("home"), app, true, b"\x1bOH");
    assert_bytes(&stroke("end"), app, true, b"\x1bOF");
    // A modifier stops the binding from matching and build_sequence wins.
    assert_bytes(&modified("up", None, shift()), app, true, b"\x1b[1;2A");
    // Alt keeps the modifier even with option_as_alt off, so the binding
    // does not match and the modified sequence wins.
    assert_bytes(&modified("up", None, alt()), app, false, b"\x1b[1;3A");
    // The binding has no ~REPORT_ALL_KEYS_AS_ESC and no
    // ~DISAMBIGUATE_ESC_CODES, so it still wins in those modes, as in
    // Alacritty.
    assert_bytes(
        &stroke("up"),
        app.union(Modes::REPORT_ALL_KEYS_AS_ESC),
        true,
        b"\x1bOA",
    );
}

#[test]
fn application_keypad_mode() {
    let keypad = Modes::APP_KEYPAD;
    let cases: &[(&str, &[u8])] = &[
        ("0", b"\x1bOp"),
        ("1", b"\x1bOq"),
        ("2", b"\x1bOr"),
        ("3", b"\x1bOs"),
        ("4", b"\x1bOt"),
        ("5", b"\x1bOu"),
        ("6", b"\x1bOv"),
        ("7", b"\x1bOw"),
        ("8", b"\x1bOx"),
        ("9", b"\x1bOy"),
        ("*", b"\x1bOj"),
        ("+", b"\x1bOk"),
        (",", b"\x1bOl"),
        ("-", b"\x1bOm"),
        (".", b"\x1bOn"),
        ("/", b"\x1bOo"),
        ("=", b"\x1bOX"),
    ];
    for (key, expected) in cases {
        let keystroke = typed(key, key);
        let press = KeyPress {
            location: KeyLocation::Numpad,
            ..KeyPress::new(&keystroke)
        };
        assert_eq!(
            encode(&press, keypad, EncoderConfig::default()).as_deref(),
            Some(*expected),
            "numpad {key} in application keypad mode"
        );
    }
    let enter = stroke("enter");
    let press = KeyPress {
        location: KeyLocation::Numpad,
        ..KeyPress::new(&enter)
    };
    assert_eq!(
        encode(&press, keypad, EncoderConfig::default()).as_deref(),
        Some(b"\x1bOM".as_slice())
    );
    // Without DECPAM, Alacritty's numpad-Enter binding sends a line feed.
    assert_eq!(
        encode(&press, Modes::NONE, EncoderConfig::default()).as_deref(),
        Some(b"\n".as_slice())
    );
}

#[test]
fn control_letters_and_punctuation() {
    let normal = Modes::NONE;
    // Alacritty's text path gets these bytes from the platform; GPUI leaves
    // `key_char` empty for a Ctrl chord, so the encoder derives them.
    assert_bytes(&modified("a", None, ctrl()), normal, true, b"\x01");
    assert_bytes(&modified("z", None, ctrl()), normal, true, b"\x1a");
    assert_bytes(&modified("[", None, ctrl()), normal, true, b"\x1b");
    assert_bytes(&modified("?", None, ctrl()), normal, true, b"\x7f");
    assert_bytes(&modified("space", Some(" "), ctrl()), normal, true, b"\0");
    // Ctrl+Alt+A keeps the control byte and gets the Meta prefix.
    assert_bytes(
        &modified(
            "a",
            None,
            Modifiers {
                control: true,
                alt: true,
                ..Modifiers::default()
            },
        ),
        normal,
        true,
        b"\x1b\x01",
    );
}

#[test]
fn alt_is_meta_or_composed() {
    let normal = Modes::NONE;
    // Option+O on macOS, with option_as_alt off: the composed character.
    assert_bytes(
        &modified("o", Some("ø"), alt()),
        normal,
        false,
        "ø".as_bytes(),
    );
    // With option_as_alt on, winit rewrites the text to the base key.
    assert_bytes(&modified("o", Some("ø"), alt()), normal, true, b"\x1bo");
    assert_bytes(
        &modified("s", Some("S"), alt_shift()),
        normal,
        true,
        b"\x1bS",
    );
    // A plain shifted letter.
    assert_bytes(&modified("s", Some("S"), shift()), normal, true, b"S");
    // Command chords are application shortcuts unless a Kitty mode reports
    // all keys.
    assert_eq!(pressed(&modified("s", None, cmd()), normal, true), None);
    assert_bytes(
        &modified("s", None, cmd()),
        Modes::REPORT_ALL_KEYS_AS_ESC,
        true,
        b"\x1b[115;9u",
    );
}

#[test]
fn alt_is_always_a_modifier_for_functional_keys() {
    // Alacritty's alt_send_esc: a named key without text keeps Alt whatever
    // the Option policy is, because it cannot be composed. With
    // option_as_alt off, Option+Up must still move by word in a shell.
    let normal = Modes::NONE;
    assert_bytes(&modified("up", None, alt()), normal, false, b"\x1b[1;3A");
    assert_bytes(&modified("down", None, alt()), normal, false, b"\x1b[1;3B");
    assert_bytes(&modified("left", None, alt()), normal, false, b"\x1b[1;3D");
    assert_bytes(&modified("right", None, alt()), normal, false, b"\x1b[1;3C");
    assert_bytes(&modified("home", None, alt()), normal, false, b"\x1b[1;3H");
    assert_bytes(&modified("end", None, alt()), normal, false, b"\x1b[1;3F");
    assert_bytes(
        &modified("pageup", None, alt()),
        normal,
        false,
        b"\x1b[5;3~",
    );
    assert_bytes(
        &modified("pagedown", None, alt()),
        normal,
        false,
        b"\x1b[6;3~",
    );
    assert_bytes(&modified("f5", None, alt()), normal, false, b"\x1b[15;3~");
    // A named key with text follows the policy instead.
    assert_bytes(&modified("enter", Some("\n"), alt()), normal, false, b"\r");
    // A character key needs the policy and exactly one character of text.
    assert_bytes(&modified("ab", Some("ab"), alt()), normal, true, b"ab");
    // The release path applies the same rule, so an event-type report keeps
    // the Alt bit too.
    let up = modified("up", None, alt());
    let release = KeyPress {
        state: KeyState::Released,
        ..KeyPress::new(&up)
    };
    assert_eq!(
        encode(
            &release,
            Modes::REPORT_EVENT_TYPES,
            EncoderConfig {
                alt_sends_escape: false
            }
        )
        .as_deref(),
        Some(b"\x1b[1;3:3A".as_slice())
    );
}

#[test]
fn kitty_disambiguation() {
    let disambiguate = Modes::DISAMBIGUATE_ESC_CODES;
    // try_build_named_normal leaves defaults out, so a cursor key stays the
    // xterm sequence even with DISAMBIGUATE alone.
    assert_bytes(&stroke("up"), disambiguate, true, b"\x1b[A");
    // try_build_control_char_or_mod.
    assert_bytes(&stroke("escape"), disambiguate, true, b"\x1b[27u");
    // Plain Enter is not ambiguous, so DISAMBIGUATE alone leaves it as
    // text; a modified Enter is disambiguated.
    assert_bytes(&stroke("enter"), disambiguate, true, b"\r");
    assert_bytes(
        &modified("enter", Some("\r"), shift()),
        disambiguate,
        true,
        b"\x1b[13;2u",
    );
    // The Backspace binding without modifiers is the only one that does not
    // exclude DISAMBIGUATE; the Shift and Alt bindings do, so those modified
    // keys are encoded by the Kitty protocol.
    assert_bytes(&stroke("backspace"), disambiguate, true, b"\x7f");
    assert_bytes(
        &modified("backspace", None, shift()),
        disambiguate,
        true,
        b"\x1b[127;2u",
    );
    assert_bytes(
        &modified("backspace", None, alt()),
        disambiguate,
        true,
        b"\x1b[127;3u",
    );
    // try_build_named_kitty: F3 uses 13~ in the Kitty protocol.
    assert_bytes(&stroke("f3"), disambiguate, true, b"\x1b[13~");
    // F1 has no Kitty entry, so the normal builder leaves out parameter 1.
    assert_bytes(&stroke("f1"), disambiguate, true, b"\x1b[P");
    // DISAMBIGUATE alone leaves ordinary text as text; REPORT_ALL_KEYS_AS_ESC
    // is what turns it into try_build_textual.
    assert_bytes(&modified("s", Some("S"), shift()), disambiguate, true, b"S");
}

#[test]
fn kitty_reports_all_keys() {
    let all = Modes::REPORT_ALL_KEYS_AS_ESC;
    assert_bytes(&modified("a", None, ctrl()), all, true, b"\x1b[97;5u");
    assert_bytes(&stroke("f13"), all, true, b"\x1b[57376u");
    assert_bytes(
        &modified("s", Some("S"), shift()),
        all,
        true,
        b"\x1b[115;2u",
    );
    // A modifier key reports itself and its modifier state; left and right
    // Shift differ, which is why the location is an input.
    let shift_key = stroke("shift");
    let left = KeyPress {
        location: KeyLocation::Left,
        ..KeyPress::new(&shift_key)
    };
    assert_eq!(
        encode(&left, all, EncoderConfig::default()).as_deref(),
        Some(b"\x1b[57441;2u".as_slice())
    );
    let right = KeyPress {
        location: KeyLocation::Right,
        ..KeyPress::new(&shift_key)
    };
    assert_eq!(
        encode(&right, all, EncoderConfig::default()).as_deref(),
        Some(b"\x1b[57447;2u".as_slice())
    );
}

#[test]
fn kitty_alternate_keys_and_associated_text() {
    let modes = Modes::REPORT_ALL_KEYS_AS_ESC.union(Modes::REPORT_ALTERNATE_KEYS);
    assert_bytes(
        &modified("s", Some("S"), shift()),
        modes,
        true,
        b"\x1b[115:83;2u",
    );
    // GPUI folds Shift into punctuation and clears the modifier; a caller
    // with the physical key supplies the base and the lost Shift.
    let punctuation = typed("!", "!");
    let press = KeyPress::new(&punctuation).with_platform_key(Some("1"), Some(true));
    assert_eq!(
        encode(
            &press,
            modes,
            EncoderConfig {
                alt_sends_escape: true
            }
        )
        .as_deref(),
        Some(b"\x1b[49:33;2u".as_slice())
    );

    let associated = Modes::REPORT_ALL_KEYS_AS_ESC.union(Modes::REPORT_ASSOCIATED_TEXT);
    assert_bytes(&typed("a", "a"), associated, true, b"\x1b[97;1;97u");
    // Shift+S reports the shifted character as associated text.
    assert_bytes(
        &modified("s", Some("S"), shift()),
        associated,
        true,
        b"\x1b[115;2;83u",
    );
}

#[test]
fn kitty_event_types() {
    let modes = Modes::REPORT_ALL_KEYS_AS_ESC.union(Modes::REPORT_EVENT_TYPES);
    let a = typed("a", "a");
    let press = KeyPress::new(&a);
    // The initial press omits the default event type 1.
    assert_eq!(
        encode(&press, modes, EncoderConfig::default()).as_deref(),
        Some(b"\x1b[97u".as_slice())
    );

    let repeat = KeyPress {
        repeat: true,
        ..KeyPress::new(&a)
    };
    assert_eq!(
        encode(&repeat, modes, EncoderConfig::default()).as_deref(),
        Some(b"\x1b[97;1:2u".as_slice())
    );

    let release = KeyPress {
        state: KeyState::Released,
        ..KeyPress::new(&a)
    };
    assert_eq!(
        encode(&release, modes, EncoderConfig::default()).as_deref(),
        Some(b"\x1b[97;1:3u".as_slice())
    );

    // With only REPORT_EVENT_TYPES, Enter is not reported: its press still
    // sends `\r`, and its release is in the dropped list.
    let events_only = Modes::REPORT_EVENT_TYPES;
    assert_eq!(
        encode(
            &KeyPress::new(&stroke("enter")),
            events_only,
            EncoderConfig::default()
        )
        .as_deref(),
        Some(b"\r".as_slice())
    );
    assert_eq!(
        encode(
            &KeyPress {
                state: KeyState::Released,
                ..KeyPress::new(&stroke("enter"))
            },
            events_only,
            EncoderConfig::default()
        ),
        None
    );

    // Up is a functional key, so event types apply without REPORT_ALL.
    assert_eq!(
        encode(
            &KeyPress::new(&stroke("up")),
            events_only,
            EncoderConfig::default()
        )
        .as_deref(),
        Some(b"\x1b[A".as_slice())
    );
    assert_eq!(
        encode(
            &KeyPress {
                state: KeyState::Released,
                ..KeyPress::new(&stroke("up"))
            },
            events_only,
            EncoderConfig::default()
        )
        .as_deref(),
        Some(b"\x1b[1;1:3A".as_slice())
    );

    // Without REPORT_EVENT_TYPES a release writes nothing.
    assert_eq!(
        encode(
            &KeyPress {
                state: KeyState::Released,
                ..KeyPress::new(&a)
            },
            Modes::NONE,
            EncoderConfig::default()
        ),
        None
    );
}

#[test]
fn kitty_numpad_codes() {
    let modes = Modes::DISAMBIGUATE_ESC_CODES;
    let one = typed("1", "1");
    let press = KeyPress {
        location: KeyLocation::Numpad,
        ..KeyPress::new(&one)
    };
    assert_eq!(
        encode(&press, modes, EncoderConfig::default()).as_deref(),
        Some(b"\x1b[57400u".as_slice())
    );
    let enter = stroke("enter");
    let press = KeyPress {
        location: KeyLocation::Numpad,
        ..KeyPress::new(&enter)
    };
    assert_eq!(
        encode(&press, modes, EncoderConfig::default()).as_deref(),
        Some(b"\x1b[57414u".as_slice())
    );
    // The application keypad SS3 sequence is only for the non-Kitty modes.
    let modes = Modes::APP_KEYPAD.union(Modes::DISAMBIGUATE_ESC_CODES);
    assert_eq!(
        encode(&press, modes, EncoderConfig::default()).as_deref(),
        Some(b"\x1b[57414u".as_slice())
    );
}

#[test]
fn non_ascii_key_names_are_text() {
    // A key such as Option+F on macOS is a character, not the start of an
    // "f<number>" function key; parsing must not slice inside the character.
    let key = typed("ƒ", "ƒ");
    assert_eq!(
        pressed(&key, Modes::NONE, true).as_deref(),
        Some("ƒ".as_bytes())
    );
}

#[test]
fn vi_mode_writes_nothing() {
    assert_eq!(pressed(&typed("a", "a"), Modes::VI, true), None);
    // The text would otherwise be encoded.
    assert_eq!(
        pressed(&typed("a", "a"), Modes::NONE, true).as_deref(),
        Some(b"a".as_slice())
    );
}

#[test]
fn modifier_keys_are_flagged() {
    let shift_key = stroke("shift");
    let press = KeyPress::new(&shift_key);
    assert!(press.is_modifier());
    let letter = typed("a", "a");
    let press = KeyPress::new(&letter);
    assert!(!press.is_modifier());
}
