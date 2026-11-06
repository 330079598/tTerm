// Derived from Alacritty's `alacritty/src/input/keyboard.rs`, commit
// 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: the input is GPUI's `Keystroke` instead of winit's `KeyEvent`,
// and the pieces Alacritty reads from its own configuration are parameters
// on `EncoderConfig` (see its documentation). The file is split into
// `named.rs` and `sequence.rs` to stay under the repository's file limit;
// the protocol bytes are unchanged. `third-party.md` records the source.

//! Keyboard encoding: GPUI's `Keystroke` to the bytes a program expects.
//!
//! The encoder is a pure function. It does not read configuration, search
//! state or the event loop; the view passes the terminal modes, the key's
//! location and the Alt policy, then writes the result to the session.
//!
//! # What is ported
//!
//! - The text path, where a named key's text or a character's typed text is
//!   sent, with an escape prefix for Meta.
//! - The application-cursor, F1-F4, Shift+Tab, Backspace and numpad-Enter
//!   bindings that Alacritty's default key bindings supply. They are part of
//!   the protocol even though Alacritty keeps them outside `build_sequence`.
//! - Alacritty's `build_sequence`: named and functional keys, control
//!   characters, textual characters, modifiers and the Kitty keyboard
//!   protocol flags.
//! - Application keypad (DECPAM) sequences, which Alacritty parses but does
//!   not encode; the sequences are xterm's.
//!
//! # What is not ported
//!
//! Vi mode input, search input, hint selection, inline search and the
//! application actions (copy, paste, scroll, font size, new window). The
//! view routes those before calling this module, as Alacritty runs its own
//! bindings before the terminal encoding.

mod named;
mod sequence;

use gpui_kit::Keystroke;
use tterm_terminal::mode::Modes;

use self::named::NamedKey;
use self::sequence::SequenceModifiers;
use self::sequence::SequenceRequest;
use self::sequence::build_sequence;

/// Whether a key event is a press or a release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyState {
    /// The key went down; the first press is not a repeat.
    Pressed,
    /// The key came up; only the Kitty protocol reports it.
    Released,
}

/// Where a key is on the keyboard.
///
/// GPUI's `Keystroke` does not expose the location, so callers that know it
/// pass it here. It decides the numeric-keypad sequences and the left-hand
/// modifier codes of the Kitty protocol.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyLocation {
    /// The usual location, or one the platform could not identify.
    #[default]
    Standard,
    /// The left member of a pair, such as the left Shift.
    Left,
    /// The right member of a pair.
    Right,
    /// The numeric keypad.
    Numpad,
}

/// What the encoder needs besides the keystroke itself.
///
/// Alacritty reads these from `UiConfig` and from the window; making them
/// parameters keeps the protocol testable and keeps configuration out of the
/// view crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncoderConfig {
    /// Whether Alt/Option is a Meta prefix: send `ESC` before the key's text
    /// instead of sending the composed character.
    ///
    /// Alacritty's default is `option_as_alt = None` on macOS, which means
    /// `false`; on the other platforms its `alt_send_esc` is always true, so
    /// the default is `true`. Callers that expose a setting set it through
    /// [`EncoderConfig::macos`] or by assigning the field.
    ///
    /// The flag only applies to keys that carry text. Alacritty always treats
    /// Alt as a modifier for a named key without text, such as an arrow or a
    /// function key, because such a key cannot be composed.
    pub alt_sends_escape: bool,
}

impl EncoderConfig {
    /// The macOS policy: Option sends `ESC` only when the user enabled
    /// `option_as_alt`, which Alacritty defaults to `None`.
    pub const fn macos(option_as_alt: bool) -> Self {
        Self {
            alt_sends_escape: option_as_alt,
        }
    }
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            alt_sends_escape: !cfg!(target_os = "macos"),
        }
    }
}

/// One key press, assembled from a GPUI event and the platform data GPUI
/// does not carry.
pub struct KeyPress<'a> {
    /// The keystroke the platform generated.
    pub keystroke: &'a Keystroke,
    /// Press or release.
    pub state: KeyState,
    /// Whether the platform marked the press as an auto-repeat.
    pub repeat: bool,
    /// The key's location, when the caller knows it.
    pub location: KeyLocation,
    /// The key before Shift and Alt were applied, when the platform exposes
    /// it separately. Alacritty gets it from winit's
    /// `key_without_modifiers`; GPUI's `Keystroke` does not carry it, so the
    /// default is `None`.
    pub base_key: Option<&'a str>,
    /// Whether Shift was held when the platform folded it into `key` and
    /// cleared `keystroke.modifiers.shift`. GPUI does that for shifted
    /// punctuation; `None` means trust the keystroke's flag.
    pub shift: Option<bool>,
}

impl<'a> KeyPress<'a> {
    /// A plain press of the keystroke: no repeat, no location, no platform
    /// data beyond what GPUI carries.
    pub fn new(keystroke: &'a Keystroke) -> Self {
        Self {
            keystroke,
            state: KeyState::Pressed,
            repeat: false,
            location: KeyLocation::Standard,
            base_key: None,
            shift: None,
        }
    }

    /// Adds the platform-only fields: the unmodified key and the Shift state
    /// the platform folded into the printed key.
    pub fn with_platform_key(mut self, base_key: Option<&'a str>, shift: Option<bool>) -> Self {
        self.base_key = base_key;
        self.shift = shift;
        self
    }

    /// Whether the key is a modifier key such as Shift or Control.
    ///
    /// Alacritty uses this to avoid clearing the selection and scrolling to
    /// the bottom when a modifier key is written.
    pub fn is_modifier(&self) -> bool {
        matches!(
            classify(&self.keystroke.key),
            Key::Named(named) if named.is_modifier()
        )
    }
}

/// A keystroke classified as a named key or a character key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Key<'a> {
    /// A key with a fixed name, such as Enter or F5.
    Named(NamedKey),
    /// A key that produces text; the string is GPUI's key, not the text.
    Character(&'a str),
}

/// Encodes one key event for the terminal.
///
/// `None` means the event is not terminal input and the view should offer it
/// to the application shortcuts. An empty vector can never be returned: a
/// key that encodes to nothing is treated as unhandled.
pub fn encode(press: &KeyPress<'_>, modes: Modes, config: EncoderConfig) -> Option<Vec<u8>> {
    // Vi mode has its own input model, which P2 does not ask this card to
    // port. Alacritty ignores the key entirely while it is active.
    if modes.contains(Modes::VI) {
        return None;
    }

    let keystroke = press.keystroke;
    let key = classify(&keystroke.key);
    let modifiers = effective_modifiers(press);
    let text = text_for(press, &key, config);

    if press.state == KeyState::Released {
        return encode_release(press, &key, &text, modifiers, modes, config);
    }

    if let Some(bytes) = encode_binding(&key, press, modifiers, modes) {
        return Some(bytes);
    }

    let alt_sends_escape = sends_alt_escape(&key, &text, modifiers, config);
    let input_modifiers = if alt_sends_escape {
        modifiers
    } else {
        // Alacritty masks Alt when it is not a Meta prefix, so modifiers,
        // event types and associated text do not report it.
        modifiers.without_alt()
    };

    let bytes = if should_build_sequence(&key, &text, modes, input_modifiers, press.location) {
        build_sequence(&SequenceRequest {
            key,
            text: &text,
            key_char: keystroke.key_char.as_deref(),
            base_key: press.base_key,
            state: press.state,
            repeat: press.repeat,
            location: press.location,
            modifiers: input_modifiers,
            modes,
        })?
    } else {
        let mut bytes = Vec::with_capacity(text.len() + 1);
        if alt_sends_escape {
            bytes.push(b'\x1b');
        }
        bytes.extend_from_slice(text.as_bytes());
        bytes
    };

    (!bytes.is_empty()).then_some(bytes)
}

/// Encodes a key release, which only the Kitty event-type mode reports.
fn encode_release(
    press: &KeyPress<'_>,
    key: &Key<'_>,
    text: &str,
    modifiers: SequenceModifiers,
    modes: Modes,
    config: EncoderConfig,
) -> Option<Vec<u8>> {
    if !modes.contains(Modes::REPORT_EVENT_TYPES) {
        return None;
    }
    // Alacritty drops releases of Enter, Tab and Backspace unless every key
    // is reported; their press already produced the text.
    if matches!(
        key,
        Key::Named(NamedKey::Enter | NamedKey::Tab | NamedKey::Backspace)
    ) && !modes.contains(Modes::REPORT_ALL_KEYS_AS_ESC)
    {
        return None;
    }

    let alt_sends_escape = sends_alt_escape(key, text, modifiers, config);
    let input_modifiers = if alt_sends_escape {
        modifiers
    } else {
        modifiers.without_alt()
    };
    let bytes = build_sequence(&SequenceRequest {
        key: *key,
        text,
        key_char: press.keystroke.key_char.as_deref(),
        base_key: press.base_key,
        state: press.state,
        repeat: press.repeat,
        location: press.location,
        modifiers: input_modifiers,
        modes,
    })?;
    (!bytes.is_empty()).then_some(bytes)
}

/// The protocol bindings Alacritty supplies outside `build_sequence`.
///
/// The mode and modifier tests are the `mode` and `mods` fields of the
/// `bindings!` table in `config/bindings.rs`. Application keypad is the one
/// extension: Alacritty parses DECPAM but has no binding for it, while
/// xterm and the old tTerm send the SS3 keypad sequences.
fn encode_binding(
    key: &Key<'_>,
    press: &KeyPress<'_>,
    modifiers: SequenceModifiers,
    modes: Modes,
) -> Option<Vec<u8>> {
    let kitty = modes.contains(Modes::REPORT_ALL_KEYS_AS_ESC)
        || modes.contains(Modes::DISAMBIGUATE_ESC_CODES)
        || modes.contains(Modes::REPORT_EVENT_TYPES);
    if modes.contains(Modes::APP_KEYPAD)
        && press.location == KeyLocation::Numpad
        && !kitty
        && modifiers.is_empty()
        && let Some(sequence) = application_keypad(key)
    {
        return Some(sequence.to_vec());
    }

    if modes.contains(Modes::APP_CURSOR)
        && modifiers.is_empty()
        && let Key::Named(named) = key
        && let Some(sequence) = named.application_cursor()
    {
        return Some(sequence.to_vec());
    }

    let reports_all = modes.contains(Modes::REPORT_ALL_KEYS_AS_ESC);
    let disambiguates = modes.contains(Modes::DISAMBIGUATE_ESC_CODES);

    if !reports_all
        && !disambiguates
        && modifiers.is_empty()
        && let Key::Named(named) = key
        && let Some(sequence) = named.legacy_function_key()
    {
        return Some(sequence.to_vec());
    }

    if !reports_all
        && !disambiguates
        && matches!(key, Key::Named(NamedKey::Tab))
        && (modifiers == SequenceModifiers::SHIFT
            || modifiers == SequenceModifiers::SHIFT.union(SequenceModifiers::ALT))
    {
        return Some(if modifiers.contains(SequenceModifiers::ALT) {
            b"\x1b\x1b[Z".to_vec()
        } else {
            b"\x1b[Z".to_vec()
        });
    }

    if !reports_all && let Key::Named(NamedKey::Backspace) = key {
        if modifiers.is_empty() || modifiers == SequenceModifiers::SHIFT {
            return Some(b"\x7f".to_vec());
        }
        if modifiers == SequenceModifiers::ALT {
            // The binding sends ESC DEL even when Option normally composes,
            // because the binding runs before the text path.
            return Some(b"\x1b\x7f".to_vec());
        }
    }

    if !reports_all
        && !disambiguates
        && press.location == KeyLocation::Numpad
        && modifiers.is_empty()
        && matches!(key, Key::Named(NamedKey::Enter))
    {
        return Some(b"\n".to_vec());
    }

    None
}

/// The SS3 sequence of a numeric-keypad key in application keypad mode.
///
/// These are xterm's DECPAM sequences (`ESC O p` through `ESC O y` for the
/// digits, the operator keys around them and `ESC O M` for Enter).
fn application_keypad(key: &Key<'_>) -> Option<&'static [u8]> {
    let sequence: &'static [u8] = match key {
        Key::Character("0") => b"\x1bOp",
        Key::Character("1") => b"\x1bOq",
        Key::Character("2") => b"\x1bOr",
        Key::Character("3") => b"\x1bOs",
        Key::Character("4") => b"\x1bOt",
        Key::Character("5") => b"\x1bOu",
        Key::Character("6") => b"\x1bOv",
        Key::Character("7") => b"\x1bOw",
        Key::Character("8") => b"\x1bOx",
        Key::Character("9") => b"\x1bOy",
        Key::Character("*") => b"\x1bOj",
        Key::Character("+") => b"\x1bOk",
        Key::Character(",") => b"\x1bOl",
        Key::Character("-") => b"\x1bOm",
        Key::Character(".") => b"\x1bOn",
        Key::Character("/") => b"\x1bOo",
        Key::Character("=") => b"\x1bOX",
        Key::Named(NamedKey::Enter) => b"\x1bOM",
        _ => return None,
    };
    Some(sequence)
}

/// Maps GPUI's key name to a named key or a character key.
fn classify(key: &str) -> Key<'_> {
    match NamedKey::from_name(key) {
        Some(named) => Key::Named(named),
        None => Key::Character(key),
    }
}

/// The modifiers the encoder should report, including a Shift the platform
/// folded into the printed key.
fn effective_modifiers(press: &KeyPress<'_>) -> SequenceModifiers {
    let mut modifiers = SequenceModifiers::from_gpui(press.keystroke.modifiers);
    if let Some(shift) = press.shift {
        modifiers.set(SequenceModifiers::SHIFT, shift);
    }
    modifiers
}

/// Whether Alt is a Meta prefix for this key, as Alacritty's `alt_send_esc`
/// decides.
///
/// A named key without text is a functional key such as an arrow or F5, and
/// Alacritty treats Alt as a modifier for it whatever the Option policy is,
/// so Option+Up keeps the Alt bit even where Option normally composes. A
/// named key with text follows the policy, and a character key needs the
/// policy plus exactly one character of text.
fn sends_alt_escape(
    key: &Key<'_>,
    text: &str,
    modifiers: SequenceModifiers,
    config: EncoderConfig,
) -> bool {
    if !modifiers.contains(SequenceModifiers::ALT) {
        return false;
    }
    match key {
        Key::Named(named) => named.text().is_none() || config.alt_sends_escape,
        Key::Character(_) => config.alt_sends_escape && text.chars().count() == 1,
    }
}

/// The text a key produces outside the escape-sequence builder.
fn text_for(press: &KeyPress<'_>, key: &Key<'_>, config: EncoderConfig) -> String {
    let keystroke = press.keystroke;
    match key {
        Key::Named(named) => {
            // xterm sends NUL for Ctrl+Space; Alacritty reaches the same
            // byte through the platform's text when it produces one.
            if *named == NamedKey::Space && keystroke.modifiers.control {
                return "\0".to_string();
            }
            named.text().unwrap_or("").to_string()
        }
        Key::Character(character) => {
            if keystroke.modifiers.control {
                if let Some(control) = keystroke
                    .key_char
                    .as_deref()
                    .and_then(single_char)
                    .filter(|character| character.is_control())
                {
                    return control.to_string();
                }
                if let Some(control) = control_character(character) {
                    return control.to_string();
                }
            }
            // On macOS a Command chord has no text; it is an application
            // shortcut, so nothing is written unless a Kitty mode asks for
            // every key.
            if keystroke.modifiers.platform && keystroke.key_char.is_none() {
                return String::new();
            }
            if keystroke.modifiers.alt && config.alt_sends_escape {
                return without_alt_text(
                    character,
                    press.shift.unwrap_or(keystroke.modifiers.shift),
                );
            }
            keystroke
                .key_char
                .as_deref()
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| character.to_string())
        }
    }
}

/// The text of a key with Alt removed, as winit's `OptionAsAlt` rewrite
/// produces it: Shift stays applied and Option does not compose.
fn without_alt_text(key: &str, shift: bool) -> String {
    if shift && key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() {
        key.to_ascii_uppercase()
    } else {
        key.to_string()
    }
}

/// The control character a Ctrl chord produces.
///
/// This reproduces the text winit hands Alacritty for Ctrl+letter and the
/// punctuation keys that have a C0 equivalent: `@`, `[`, `\`, `]`, `^`, `_`
/// and `?`. Other keys have no C0 form and fall back to their text.
fn control_character(key: &str) -> Option<char> {
    let character = single_char(key)?;
    let code = match character {
        'a'..='z' => character as u8 & 0x1f,
        'A'..='Z' => character as u8 & 0x1f,
        '@' | ' ' => 0x00,
        '[' => 0x1b,
        '\\' => 0x1c,
        ']' => 0x1d,
        '^' => 0x1e,
        '_' => 0x1f,
        '?' => 0x7f,
        _ => return None,
    };
    char::from_u32(u32::from(code))
}

/// Alacritty's `should_build_sequence`: whether the key becomes an escape
/// sequence instead of its text.
fn should_build_sequence(
    key: &Key<'_>,
    text: &str,
    modes: Modes,
    modifiers: SequenceModifiers,
    location: KeyLocation,
) -> bool {
    if modes.contains(Modes::REPORT_ALL_KEYS_AS_ESC) {
        return true;
    }

    let disambiguate = modes.contains(Modes::DISAMBIGUATE_ESC_CODES)
        && (matches!(key, Key::Named(NamedKey::Escape))
            || location == KeyLocation::Numpad
            || (!modifiers.is_empty()
                && (modifiers != SequenceModifiers::SHIFT
                    || matches!(
                        key,
                        Key::Named(NamedKey::Tab | NamedKey::Enter | NamedKey::Backspace)
                    ))));

    match key {
        _ if disambiguate => true,
        // Named keys without text are functional keys, not input text.
        Key::Named(named) => named.text().is_none(),
        Key::Character(_) => text.is_empty(),
    }
}

/// The first character of `text`, if it has exactly one.
fn single_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(first)
}

#[cfg(test)]
mod tests;
