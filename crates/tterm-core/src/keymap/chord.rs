//! Platform-neutral keyboard chord parsing and formatting.
use std::fmt;
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Chord {
    pub key: String,
    pub mod_key: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}
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
    pub editable: bool,
}
const MODIFIERS: &[&str] = &["mod", "ctrl", "alt", "shift"];
fn valid_named(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}
fn valid_single(s: &str) -> bool {
    s.len() == 1
        && s.chars().next().is_some_and(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || "\\/-[]`;=',.".contains(c)
        })
}
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
pub fn parse_chord(s: &str) -> Option<Chord> {
    let p: Vec<_> = s
        .split('+')
        .map(str::trim)
        .map(str::to_lowercase)
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
pub fn is_editable_target(t: EditableTarget) -> bool {
    t.editable
}
impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&serialize_chord(self).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_round_trips_unicode_input_by_physical_code() {
        assert_eq!(normalize_event_key("P"), Some("p".into()));
        assert_eq!(normalize_event_key("Tab"), Some("tab".into()));
        assert_eq!(normalize_event_key("+"), Some("plus".into()));
        let event = KeyEventData {
            key: "е",
            code: Some("KeyT"),
            meta: false,
            ctrl: true,
            alt: false,
            shift: false,
            composing: false,
        };
        assert_eq!(
            event_to_chord(event, false),
            Some(Chord {
                key: "t".into(),
                mod_key: true,
                ..Chord::default()
            })
        );
    }

    #[test]
    fn parses_serializes_and_formats_platform_chords() {
        let chord = parse_chord("SHIFT+mod+P").unwrap();
        assert_eq!(serialize_chord(&chord).as_deref(), Some("mod+shift+p"));
        assert_eq!(format_chord("mod+shift+p", true), "⌘⇧P");
        assert_eq!(format_chord("mod+shift+p", false), "Ctrl+Shift+P");
        assert!(parse_chord("mod+shift+bogus+key").is_none());
    }

    #[test]
    fn rejects_composition_and_modifier_events() {
        let base = KeyEventData {
            key: "Control",
            code: None,
            meta: false,
            ctrl: false,
            alt: false,
            shift: false,
            composing: false,
        };
        assert!(event_to_chord(base, false).is_none());
        assert!(
            event_to_chord(
                KeyEventData {
                    key: "x",
                    composing: true,
                    ..base
                },
                false
            )
            .is_none()
        );
    }
}
