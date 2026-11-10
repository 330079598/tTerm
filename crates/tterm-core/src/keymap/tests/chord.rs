use crate::keymap::chord::*;

#[test]
fn normalize_event_key_normalizes_letters_to_lowercase() {
    assert_eq!(normalize_event_key("P"), Some("p".into()));
    assert_eq!(normalize_event_key("p"), Some("p".into()));
}
#[test]
fn normalize_event_key_normalizes_named_keys() {
    assert_eq!(normalize_event_key("Tab"), Some("tab".into()));
    assert_eq!(normalize_event_key("PageDown"), Some("pagedown".into()));
    assert_eq!(normalize_event_key("F5"), Some("f5".into()));
}
#[test]
fn normalize_event_key_aliases_keys_that_would_break_plus_serialization() {
    assert_eq!(normalize_event_key(" "), Some("space".into()));
    assert_eq!(normalize_event_key("+"), Some("plus".into()));
}
#[test]
fn normalize_event_key_keeps_bindable_punctuation() {
    assert_eq!(normalize_event_key("\\"), Some("\\".into()));
    assert_eq!(normalize_event_key("`"), Some("`".into()));
    assert_eq!(normalize_event_key("]"), Some("]".into()));
}
#[test]
fn normalize_event_key_rejects_modifiers_and_unbindable_keys() {
    for key in ["Shift", "Control", "Meta", "Dead", ""] {
        assert_eq!(normalize_event_key(key), None, "key={key}");
    }
}
#[test]
fn normalize_event_code_maps_physical_letter_digit_and_punctuation_keys() {
    assert_eq!(normalize_event_code("KeyT"), Some("t".into()));
    assert_eq!(normalize_event_code("Digit1"), Some("1".into()));
    assert_eq!(normalize_event_code("Backslash"), Some("\\".into()));
    assert_eq!(normalize_event_code("PageDown"), Some("pagedown".into()));
}
#[test]
fn serialize_chord_round_trips_chords_in_canonical_order() {
    let c = Chord {
        key: "p".into(),
        mod_key: true,
        shift: true,
        ..Default::default()
    };
    let serialized = serialize_chord(&c);
    assert_eq!(serialized.as_deref(), Some("mod+shift+p"));
    assert_eq!(parse_chord(serialized.as_deref().unwrap()), Some(c));
}
#[test]
fn parse_chord_parses_serialized_chords() {
    assert_eq!(
        parse_chord("mod+f"),
        Some(Chord {
            key: "f".into(),
            mod_key: true,
            ..Default::default()
        })
    );
    assert_eq!(
        parse_chord("alt+1"),
        Some(Chord {
            key: "1".into(),
            alt: true,
            ..Default::default()
        })
    );
    assert_eq!(
        parse_chord("mod+tab"),
        Some(Chord {
            key: "tab".into(),
            mod_key: true,
            ..Default::default()
        })
    );
    assert_eq!(
        parse_chord("ctrl+tab"),
        Some(Chord {
            key: "tab".into(),
            ctrl: true,
            ..Default::default()
        })
    );
    assert_eq!(
        parse_chord("mod+shift+\\"),
        Some(Chord {
            key: "\\".into(),
            mod_key: true,
            shift: true,
            ..Default::default()
        })
    );
}
#[test]
fn parse_chord_rejects_malformed_serializations() {
    for value in ["", "mod+shift+bogus+key", "shift+"] {
        assert_eq!(parse_chord(value), None, "value={value}");
    }
}
#[test]
fn event_to_chord_maps_ctrl_to_mod_on_non_mac_platforms() {
    let e = KeyEventData {
        key: "p",
        code: None,
        meta: false,
        ctrl: true,
        alt: false,
        shift: false,
        composing: false,
    };
    assert_eq!(
        event_to_chord(e, false),
        Some(Chord {
            key: "p".into(),
            mod_key: true,
            ..Default::default()
        })
    );
}
#[test]
fn event_to_chord_maps_meta_and_ctrl_to_mod_on_mac() {
    let base = KeyEventData {
        key: "p",
        code: None,
        meta: false,
        ctrl: false,
        alt: false,
        shift: false,
        composing: false,
    };
    assert_eq!(
        event_to_chord(KeyEventData { meta: true, ..base }, true),
        Some(Chord {
            key: "p".into(),
            mod_key: true,
            ..Default::default()
        })
    );
    assert_eq!(
        event_to_chord(KeyEventData { ctrl: true, ..base }, true),
        Some(Chord {
            key: "p".into(),
            ctrl: true,
            ..Default::default()
        })
    );
}
#[test]
fn event_to_chord_captures_alt_and_shift() {
    let e = KeyEventData {
        key: "p",
        code: None,
        meta: false,
        ctrl: true,
        alt: true,
        shift: true,
        composing: false,
    };
    assert_eq!(
        event_to_chord(e, false),
        Some(Chord {
            key: "p".into(),
            mod_key: true,
            alt: true,
            shift: true,
            ..Default::default()
        })
    );
}
#[test]
fn event_to_chord_uses_physical_key_codes_across_non_latin_layouts() {
    let russian = KeyEventData {
        key: "е",
        code: Some("KeyT"),
        meta: false,
        ctrl: true,
        alt: false,
        shift: false,
        composing: false,
    };
    let french = KeyEventData {
        key: "&",
        code: Some("Digit1"),
        meta: false,
        ctrl: false,
        alt: true,
        shift: false,
        composing: false,
    };
    assert_eq!(
        event_to_chord(russian, false),
        Some(Chord {
            key: "t".into(),
            mod_key: true,
            ..Default::default()
        })
    );
    assert_eq!(
        event_to_chord(french, false),
        Some(Chord {
            key: "1".into(),
            alt: true,
            ..Default::default()
        })
    );
}
#[test]
fn event_to_chord_keeps_command_and_physical_control_distinct_on_macos() {
    let base = KeyEventData {
        key: "е",
        code: Some("KeyT"),
        meta: false,
        ctrl: false,
        alt: false,
        shift: false,
        composing: false,
    };
    assert_eq!(
        event_to_chord(KeyEventData { meta: true, ..base }, true),
        Some(Chord {
            key: "t".into(),
            mod_key: true,
            ..Default::default()
        })
    );
    assert_eq!(
        event_to_chord(KeyEventData { ctrl: true, ..base }, true),
        Some(Chord {
            key: "t".into(),
            ctrl: true,
            ..Default::default()
        })
    );
}
#[test]
fn event_to_chord_does_not_dispatch_ime_composition_or_dead_key_intermediates() {
    let b = KeyEventData {
        key: "Process",
        code: Some("KeyT"),
        meta: false,
        ctrl: false,
        alt: false,
        shift: false,
        composing: false,
    };
    assert_eq!(event_to_chord(b, false), None);
    let b = KeyEventData {
        key: "Dead",
        code: Some("Quote"),
        ..b
    };
    assert_eq!(event_to_chord(b, false), None);
    let b = KeyEventData {
        key: "p",
        code: None,
        composing: true,
        ..b
    };
    assert_eq!(event_to_chord(b, false), None);
}
#[test]
fn event_to_chord_returns_null_for_bare_modifiers() {
    assert_eq!(
        event_to_chord(
            KeyEventData {
                key: "Control",
                code: None,
                meta: false,
                ctrl: false,
                alt: false,
                shift: false,
                composing: false
            },
            false
        ),
        None
    );
}
#[test]
fn format_chord_renders_mac_style_symbols() {
    assert_eq!(format_chord("mod+shift+p", true), "⌘⇧P");
    assert_eq!(format_chord("mod+tab", true), "⌘Tab");
    assert_eq!(format_chord("alt+1", true), "⌥1");
    assert_eq!(format_chord("ctrl+tab", true), "⌃Tab");
}
#[test]
fn format_chord_renders_windows_style_labels() {
    assert_eq!(format_chord("mod+shift+p", false), "Ctrl+Shift+P");
    assert_eq!(format_chord("mod+tab", false), "Ctrl+Tab");
    assert_eq!(format_chord("alt+1", false), "Alt+1");
}
#[test]
fn format_chord_falls_back_to_raw_serialization_for_unparseable_chords() {
    assert_eq!(format_chord("nonsense+key", false), "nonsense+key");
}
#[test]
fn is_editable_target_detects_input_textarea_and_contenteditable() {
    assert!(is_editable_target(EditableTarget {
        form_control: true,
        content_editable: false,
        editable_ancestor: false
    }));
}
#[test]
fn is_editable_target_detects_editable_ancestors() {
    assert!(is_editable_target(EditableTarget {
        form_control: false,
        content_editable: false,
        editable_ancestor: true
    }));
}
#[test]
fn is_editable_target_rejects_plain_elements() {
    for target in [
        EditableTarget {
            form_control: false,
            content_editable: false,
            editable_ancestor: false,
        },
        EditableTarget {
            form_control: false,
            content_editable: false,
            editable_ancestor: false,
        },
    ] {
        assert!(!is_editable_target(target));
    }
}
