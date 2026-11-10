//! Platform-neutral keyboard chord parsing and formatting.
use std::fmt;
/// Canonical keyboard chord with normalized key and modifier flags.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Chord {
    pub key: String,
    pub mod_key: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}
/// Platform-neutral keyboard event data supplied by an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEventData<'a> {
    pub key: &'a str,
    pub code: Option<&'a str>,
    pub meta: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub composing: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditableTarget {
    /// Whether the target itself is an input or textarea control.
    pub form_control: bool,
    /// Whether the target itself has contenteditable or textbox semantics.
    pub content_editable: bool,
    /// Whether an ancestor supplies editable semantics.
    pub editable_ancestor: bool,
}
const MODIFIERS: &[&str] = &["mod", "ctrl", "alt", "shift"];
fn js_trim(value: &str) -> String {
    value
        .trim_matches(|character: char| {
            matches!(character,
        '\u{0009}'..='\u{000D}' | '\u{0020}' | '\u{00A0}' | '\u{1680}' |
        '\u{2000}'..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' |
        '\u{205F}' | '\u{3000}' | '\u{FEFF}')
        })
        .to_owned()
}

fn valid_named(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}
fn valid_single(s: &str) -> bool {
    s.len() == 1
        && s.chars().next().is_some_and(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || "\\/-[]`;=',=.".contains(c)
        })
}
/// Normalizes a logical key into the persisted key token.
pub fn normalize_event_key(raw: &str) -> Option<String> {
    if raw.is_empty()
        || matches!(
            raw,
            "Shift" | "Control" | "Alt" | "Meta" | "Dead" | "Unidentified" | "Process" | "Compose"
        )
    {
        return None;
    };
    Some(match raw {
        " " => "space".into(),
        "+" => "plus".into(),
        _ => {
            let l = raw.to_lowercase();
            if l.chars().count() == 1 {
                if valid_single(&l) { l } else { return None }
            } else if valid_named(&l) {
                l
            } else {
                return None;
            }
        }
    })
}
/// Normalizes a physical keyboard code into the persisted key token.
pub fn normalize_event_code(raw: &str) -> Option<String> {
    if raw.starts_with("Key") && raw.len() == 4 && raw.as_bytes()[3].is_ascii_uppercase() {
        return Some(raw[3..].to_lowercase());
    }
    if raw.starts_with("Digit") && raw.len() == 6 && raw.as_bytes()[5].is_ascii_digit() {
        return Some(raw[5..].into());
    }
    let a = match raw {
        "Space" => Some("space"),
        "Backquote" => Some("`"),
        "Minus" => Some("-"),
        "Equal" => Some("="),
        "BracketLeft" => Some("["),
        "BracketRight" => Some("]"),
        "Backslash" => Some("\\"),
        "Semicolon" => Some(";"),
        "Quote" => Some("'"),
        "Comma" => Some(","),
        "Period" => Some("."),
        "Slash" => Some("/"),
        _ => None,
    };
    a.map(str::to_owned).or_else(|| normalize_event_key(raw))
}
/// Converts platform-neutral event data into a canonical chord.
pub fn event_to_chord(event: KeyEventData<'_>, mac: bool) -> Option<Chord> {
    if event.composing
        || matches!(
            event.key,
            "Dead" | "Unidentified" | "Process" | "Compose" | "Shift" | "Control" | "Alt" | "Meta"
        )
    {
        return None;
    }
    let key = event
        .code
        .and_then(normalize_event_code)
        .or_else(|| normalize_event_key(event.key))?;
    Some(Chord {
        key,
        mod_key: if mac { event.meta } else { event.ctrl },
        ctrl: mac && event.ctrl,
        alt: event.alt,
        shift: event.shift,
    })
}
/// Serializes a chord using the persisted modifier order.
pub fn serialize_chord(chord: &Chord) -> Option<String> {
    if chord.key.is_empty() {
        return None;
    }
    let mut p = Vec::new();
    if chord.mod_key {
        p.push("mod")
    }
    if chord.ctrl {
        p.push("ctrl")
    }
    if chord.alt {
        p.push("alt")
    }
    if chord.shift {
        p.push("shift")
    }
    p.push(&chord.key);
    Some(p.join("+"))
}
/// Parses a persisted chord string using ECMAScript-compatible trimming.
pub fn parse_chord(serialized: &str) -> Option<Chord> {
    let parts: Vec<_> = serialized
        .split('+')
        .map(js_trim)
        .map(|part| part.to_lowercase())
        .filter(|x| !x.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    let mut chord = Chord::default();
    for x in &parts[..parts.len() - 1] {
        match x.as_str() {
            "mod" => {
                if chord.mod_key {
                    return None;
                } else {
                    chord.mod_key = true
                }
            }
            "ctrl" => {
                if chord.ctrl {
                    return None;
                } else {
                    chord.ctrl = true
                }
            }
            "alt" => {
                if chord.alt {
                    return None;
                } else {
                    chord.alt = true
                }
            }
            "shift" => {
                if chord.shift {
                    return None;
                } else {
                    chord.shift = true
                }
            }
            _ => return None,
        }
    }
    let key_token = &parts[parts.len() - 1];
    if MODIFIERS.contains(&key_token.as_str())
        || !(valid_single(key_token) || valid_named(key_token))
    {
        return None;
    }
    chord.key = key_token.clone();
    Some(chord)
}
fn display_key(k: &str) -> String {
    match k {
        "space" => "Space".into(),
        "plus" => "+".into(),
        "escape" => "Esc".into(),
        "tab" => "Tab".into(),
        "enter" => "Enter".into(),
        "backspace" => "Backspace".into(),
        "delete" => "Del".into(),
        "home" => "Home".into(),
        "end" => "End".into(),
        "pagedown" => "PageDown".into(),
        "pageup" => "PageUp".into(),
        "insert" => "Ins".into(),
        "arrowup" => "↑".into(),
        "arrowdown" => "↓".into(),
        "arrowleft" => "←".into(),
        "arrowright" => "→".into(),
        "numpadadd" => "Num+".into(),
        "numpadsubtract" => "Num-".into(),
        "numpad0" => "Num0".into(),
        _ => {
            if k.len() == 1 {
                k.to_uppercase()
            } else {
                k.into()
            }
        }
    }
}
/// Formats a persisted chord for the requested platform.
pub fn format_chord(serialized: &str, mac: bool) -> String {
    let Some(chord) = parse_chord(serialized) else {
        return serialized.into();
    };
    let key_label = display_key(&chord.key);
    if mac {
        format!(
            "{}{}{}{}{}",
            if chord.mod_key { "⌘" } else { "" },
            if chord.ctrl { "⌃" } else { "" },
            if chord.alt { "⌥" } else { "" },
            if chord.shift { "⇧" } else { "" },
            key_label
        )
    } else {
        format!(
            "{}{}{}{}",
            if chord.mod_key || chord.ctrl {
                "Ctrl+"
            } else {
                ""
            },
            if chord.alt { "Alt+" } else { "" },
            if chord.shift { "Shift+" } else { "" },
            key_label
        )
    }
}
/// Reports whether any editable target facet is present.
pub fn is_editable_target(target: EditableTarget) -> bool {
    target.form_control || target.content_editable || target.editable_ancestor
}
impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&serialize_chord(self).unwrap_or_default())
    }
}
