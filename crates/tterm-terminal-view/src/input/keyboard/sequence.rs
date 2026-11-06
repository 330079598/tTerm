// Derived from Alacritty's `alacritty/src/input/keyboard.rs`, commit
// 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: split out of `keyboard.rs`; the protocol logic is unchanged.
//! The escape-sequence builder, ported from Alacritty's `input/keyboard.rs`.
//!
//! Alacritty's version reads winit's `KeyEvent`; this one reads the fields
//! [`super::KeyPress`] extracted from a GPUI `Keystroke`. The order in which
//! the builders are tried, the parameters they emit and the Kitty protocol
//! rules are unchanged. The source header is on the parent module and in
//! `docs/gpui-migration/third-party.md`.

use tterm_terminal::mode::Modes;

use super::Key;
use super::KeyLocation;
use super::KeyState;
use super::named::NamedKey;

/// The end of an escape sequence: xterm's terminator or Kitty's `u`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Terminator {
    /// The terminator of xterm and DEC sequences.
    Normal(char),
    /// The Kitty keyboard protocol's own terminator.
    Kitty,
}

impl Terminator {
    fn as_char(self) -> char {
        match self {
            Self::Normal(character) => character,
            Self::Kitty => 'u',
        }
    }
}

/// The modifier bits of the Kitty keyboard protocol.
///
/// The order is the protocol's: shift, alt, control, super. Caps Lock and
/// Num Lock are defined by the protocol but not reported by winit, so
/// Alacritty leaves them out; so does this port.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SequenceModifiers(u8);

impl SequenceModifiers {
    pub(super) const SHIFT: Self = Self(0b0000_0001);
    pub(super) const ALT: Self = Self(0b0000_0010);
    pub(super) const CONTROL: Self = Self(0b0000_0100);
    pub(super) const SUPER: Self = Self(0b0000_1000);

    /// Builds the modifiers from GPUI's state.
    pub(super) fn from_gpui(modifiers: gpui_kit::Modifiers) -> Self {
        let mut result = Self::default();
        result.set(Self::SHIFT, modifiers.shift);
        result.set(Self::ALT, modifiers.alt);
        result.set(Self::CONTROL, modifiers.control);
        result.set(Self::SUPER, modifiers.platform);
        result
    }

    /// Adds or removes one bit.
    pub(super) fn set(&mut self, flag: Self, on: bool) {
        if on {
            self.0 |= flag.0;
        } else {
            self.0 &= !flag.0;
        }
    }

    /// Whether a bit is set.
    pub(super) fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }

    /// Whether no modifier is set.
    pub(super) fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The union of two modifier sets.
    pub(super) fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// The same flags with Alt removed.
    ///
    /// Alacritty masks Alt when it will not send an escape for it, so the
    /// composed character is reported without a Meta modifier.
    pub(super) fn without_alt(self) -> Self {
        Self(self.0 & !Self::ALT.0)
    }

    /// The value that follows `;` in an escape sequence.
    pub(super) fn encoded(self) -> u8 {
        self.0 + 1
    }
}

/// What the sequence builder needs from a key press.
pub(super) struct SequenceRequest<'key, 'text> {
    /// The key, classified as named or character.
    pub key: Key<'key>,
    /// The text the key produces, or an empty string.
    pub text: &'text str,
    /// The typed character when the platform reports one.
    pub key_char: Option<&'key str>,
    /// The base key when the platform reports one separately from `key`.
    pub base_key: Option<&'key str>,
    /// Press or release.
    pub state: KeyState,
    /// Whether the platform marked this as an auto-repeat.
    pub repeat: bool,
    /// The key's physical location.
    pub location: KeyLocation,
    /// The modifiers after the caller has applied the Alt policy.
    pub modifiers: SequenceModifiers,
    /// The terminal modes in effect.
    pub modes: Modes,
}

/// One sequence before the modifiers and event type are appended.
struct SequenceBase {
    payload: String,
    terminator: Terminator,
}

impl SequenceBase {
    fn new(payload: String, terminator: Terminator) -> Self {
        Self {
            payload,
            terminator,
        }
    }
}

/// Builds a key's escape sequence.
///
/// This is Alacritty's `build_sequence`: try the numpad, then functional
/// named keys in the Kitty protocol, then the normal named keys, then control
/// characters and modifiers, then textual characters. `None` means the key
/// has no sequence, which the caller treats as an unhandled shortcut.
pub(super) fn build_sequence(request: &SequenceRequest<'_, '_>) -> Option<Vec<u8>> {
    let kitty_sequence = request.modes.contains(Modes::REPORT_ALL_KEYS_AS_ESC)
        || request.modes.contains(Modes::DISAMBIGUATE_ESC_CODES)
        || request.modes.contains(Modes::REPORT_EVENT_TYPES);
    let kitty_encode_all = request.modes.contains(Modes::REPORT_ALL_KEYS_AS_ESC);
    let kitty_event_type = request.modes.contains(Modes::REPORT_EVENT_TYPES)
        && (request.repeat || request.state == KeyState::Released);
    let mut modifiers = request.modifiers;

    let associated_text = associated_text(request);
    let context = Builder {
        request,
        kitty_sequence,
        kitty_encode_all,
        kitty_event_type,
    };

    let base = context
        .build_numpad()
        .or_else(|| context.build_named_kitty())
        .or_else(|| context.build_named_normal(associated_text.is_some()))
        .or_else(|| context.build_control_or_modifier(&mut modifiers))
        .or_else(|| context.build_textual(associated_text.is_some()))?;

    let mut payload = format!("\x1b[{}", base.payload);
    if kitty_event_type || !modifiers.is_empty() || associated_text.is_some() {
        payload.push(';');
        payload.push_str(&modifiers.encoded().to_string());
    }
    if kitty_event_type {
        payload.push(':');
        payload.push(event_type(request));
    }
    if let Some(text) = associated_text {
        let mut codepoints = text.chars().map(u32::from);
        if let Some(codepoint) = codepoints.next() {
            payload.push(';');
            payload.push_str(&codepoint.to_string());
        }
        for codepoint in codepoints {
            payload.push(':');
            payload.push_str(&codepoint.to_string());
        }
    }
    payload.push(base.terminator.as_char());
    Some(payload.into_bytes())
}

/// The text reported as the key's associated text, if the mode asks for it.
fn associated_text<'key, 'text>(request: &SequenceRequest<'key, 'text>) -> Option<&'text str> {
    request
        .modes
        .contains(Modes::REPORT_ASSOCIATED_TEXT)
        .then_some(())
        .filter(|_| request.state != KeyState::Released)
        .filter(|_| !request.text.is_empty())
        .filter(|_| !is_control_character(request.text))
        .map(|_| request.text)
}

/// The Kitty event type: repeat, press, release.
fn event_type(request: &SequenceRequest<'_, '_>) -> char {
    if request.repeat {
        '2'
    } else if request.state == KeyState::Released {
        '3'
    } else {
        '1'
    }
}

/// Whether the text is a single C0, DEL or C1 control byte.
fn is_control_character(text: &str) -> bool {
    let Some(byte) = text.as_bytes().first() else {
        return false;
    };
    text.len() == 1 && (*byte < 0x20 || (0x7f..=0x9f).contains(byte))
}

/// The state shared by the individual builders.
struct Builder<'a, 'key, 'text> {
    request: &'a SequenceRequest<'key, 'text>,
    kitty_sequence: bool,
    kitty_encode_all: bool,
    kitty_event_type: bool,
}

impl Builder<'_, '_, '_> {
    /// Alacritty's `try_build_numpad`: the Kitty codes of the numeric keypad.
    ///
    /// The values and the order are unchanged; the comma is absent there too,
    /// so it falls through to the textual encoding.
    fn build_numpad(&self) -> Option<SequenceBase> {
        let Key::Character(character) = self.request.key else {
            return self.build_numpad_named();
        };
        if !self.kitty_sequence || self.request.location != KeyLocation::Numpad {
            return None;
        }
        let code = match character {
            "0" => "57399",
            "1" => "57400",
            "2" => "57401",
            "3" => "57402",
            "4" => "57403",
            "5" => "57404",
            "6" => "57405",
            "7" => "57406",
            "8" => "57407",
            "9" => "57408",
            "." => "57409",
            "/" => "57410",
            "*" => "57411",
            "-" => "57412",
            "+" => "57413",
            "=" => "57415",
            _ => return None,
        };
        Some(SequenceBase::new(code.to_string(), Terminator::Kitty))
    }

    /// The named half of [`Builder::build_numpad`].
    fn build_numpad_named(&self) -> Option<SequenceBase> {
        if !self.kitty_sequence || self.request.location != KeyLocation::Numpad {
            return None;
        }
        let Key::Named(named) = self.request.key else {
            return None;
        };
        let code = match named {
            NamedKey::Enter => "57414",
            NamedKey::ArrowLeft => "57417",
            NamedKey::ArrowRight => "57418",
            NamedKey::ArrowUp => "57419",
            NamedKey::ArrowDown => "57420",
            NamedKey::PageUp => "57421",
            NamedKey::PageDown => "57422",
            NamedKey::Home => "57423",
            NamedKey::End => "57424",
            NamedKey::Insert => "57425",
            NamedKey::Delete => "57426",
            _ => return None,
        };
        Some(SequenceBase::new(code.to_string(), Terminator::Kitty))
    }

    /// Alacritty's `try_build_named_kitty`.
    fn build_named_kitty(&self) -> Option<SequenceBase> {
        if !self.kitty_sequence {
            return None;
        }
        let Key::Named(named) = self.request.key else {
            return None;
        };
        let (code, terminator) = named.kitty_sequence()?;
        Some(SequenceBase::new(code.to_string(), terminator))
    }

    /// Alacritty's `try_build_named_normal`.
    fn build_named_normal(&self, has_associated_text: bool) -> Option<SequenceBase> {
        let Key::Named(named) = self.request.key else {
            return None;
        };
        // The default parameter is 1, so it is omitted when nothing else is
        // reported; a modifier, an event type or associated text needs it.
        let one_based = if self.request.modifiers.is_empty()
            && !self.kitty_event_type
            && !has_associated_text
        {
            ""
        } else {
            "1"
        };
        let (payload, terminator) = named.normal_sequence(one_based)?;
        Some(SequenceBase::new(payload, terminator))
    }

    /// Alacritty's `try_build_control_char_or_mod`.
    fn build_control_or_modifier(&self, modifiers: &mut SequenceModifiers) -> Option<SequenceBase> {
        if !self.kitty_encode_all && !self.kitty_sequence {
            return None;
        }
        let Key::Named(named) = self.request.key else {
            return None;
        };

        let mut base = named.control_character_code().unwrap_or("");
        if !self.kitty_encode_all && base.is_empty() {
            return None;
        }
        if let Some(code) = named.modifier_code(self.request.location) {
            base = code;
        }

        // Kitty applies the modifier state before the key press, while the
        // platform reports the state after it, so Alacritty derives a
        // modifier key's own state from the key itself.
        let pressed = self.request.state == KeyState::Pressed;
        match named {
            NamedKey::Shift => modifiers.set(SequenceModifiers::SHIFT, pressed),
            NamedKey::Control => modifiers.set(SequenceModifiers::CONTROL, pressed),
            NamedKey::Alt => modifiers.set(SequenceModifiers::ALT, pressed),
            NamedKey::Super => modifiers.set(SequenceModifiers::SUPER, pressed),
            _ => (),
        }

        (!base.is_empty()).then(|| SequenceBase::new(base.to_string(), Terminator::Kitty))
    }

    /// Alacritty's `try_build_textual`.
    fn build_textual(&self, has_associated_text: bool) -> Option<SequenceBase> {
        if !self.kitty_sequence {
            return None;
        }
        let Key::Character(key) = self.request.key else {
            return None;
        };
        let Some(printed) = single_char(key) else {
            // A multi-character key can still be reported through its text;
            // Kitty has the special payload 0 for exactly that case.
            return (self.kitty_encode_all && has_associated_text)
                .then(|| SequenceBase::new("0".to_string(), Terminator::Kitty));
        };
        let shift = self.request.modifiers.contains(SequenceModifiers::SHIFT);
        let base = self
            .request
            .base_key
            .and_then(single_char)
            .unwrap_or(printed);
        let unshifted = if shift {
            base.to_lowercase().next().unwrap_or(base)
        } else {
            base
        };
        let alternate = self
            .request
            .key_char
            .and_then(single_char)
            .unwrap_or(printed);
        let alternate_code = u32::from(alternate);
        let unicode_code = u32::from(unshifted);
        let payload = if self.request.modes.contains(Modes::REPORT_ALTERNATE_KEYS)
            && alternate_code != unicode_code
        {
            format!("{unicode_code}:{alternate_code}")
        } else {
            unicode_code.to_string()
        };
        Some(SequenceBase::new(payload, Terminator::Kitty))
    }
}

/// The first character of `text`, if it has exactly one.
fn single_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(first)
}
