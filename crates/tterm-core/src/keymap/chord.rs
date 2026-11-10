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
pub fn event_to_chord(e: KeyEventData<'_>, mac: bool) -> Option<Chord> {
    if e.composing
        || matches!(
            e.key,
            "Dead" | "Unidentified" | "Process" | "Compose" | "Shift" | "Control" | "Alt" | "Meta"
        )
    {
        return None;
    }
    let key = e
        .code
        .and_then(normalize_event_code)
        .or_else(|| normalize_event_key(e.key))?;
    Some(Chord {
        key,
        mod_key: if mac { e.meta } else { e.ctrl },
        ctrl: mac && e.ctrl,
        alt: e.alt,
        shift: e.shift,
    })
}
/// Serializes a chord using the persisted modifier order.
pub fn serialize_chord(c: &Chord) -> Option<String> {
    if c.key.is_empty() {
        return None;
    }
    let mut p = Vec::new();
    if c.mod_key {
        p.push("mod")
    }
    if c.ctrl {
        p.push("ctrl")
    }
    if c.alt {
        p.push("alt")
    }
    if c.shift {
        p.push("shift")
    }
    p.push(&c.key);
    Some(p.join("+"))
}
/// Parses a persisted chord string using ECMAScript-compatible trimming.
pub fn parse_chord(s: &str) -> Option<Chord> {
    let p: Vec<_> = s
        .split('+')
        .map(js_trim)
        .map(|part| part.to_lowercase())
        .filter(|x| !x.is_empty())
        .collect();
    if p.is_empty() {
        return None;
    }
    let mut c = Chord::default();
    for x in &p[..p.len() - 1] {
        match x.as_str() {
            "mod" => {
                if c.mod_key {
                    return None;
                } else {
                    c.mod_key = true
                }
            }
            "ctrl" => {
                if c.ctrl {
                    return None;
                } else {
                    c.ctrl = true
                }
            }
            "alt" => {
                if c.alt {
                    return None;
                } else {
                    c.alt = true
                }
            }
            "shift" => {
                if c.shift {
                    return None;
                } else {
                    c.shift = true
                }
            }
            _ => return None,
        }
    }
    let k = &p[p.len() - 1];
    if MODIFIERS.contains(&k.as_str()) || !(valid_single(k) || valid_named(k)) {
        return None;
    }
    c.key = k.clone();
    Some(c)
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
pub fn format_chord(s: &str, mac: bool) -> String {
    let Some(c) = parse_chord(s) else {
        return s.into();
    };
    let k = display_key(&c.key);
    if mac {
        format!(
            "{}{}{}{}{}",
            if c.mod_key { "⌘" } else { "" },
            if c.ctrl { "⌃" } else { "" },
            if c.alt { "⌥" } else { "" },
            if c.shift { "⇧" } else { "" },
            k
        )
    } else {
        format!(
            "{}{}{}{}",
            if c.mod_key || c.ctrl { "Ctrl+" } else { "" },
            if c.alt { "Alt+" } else { "" },
            if c.shift { "Shift+" } else { "" },
            k
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
