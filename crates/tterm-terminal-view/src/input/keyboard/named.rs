// Derived from Alacritty's `alacritty/src/input/keyboard.rs`, commit
// 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: split out of `keyboard.rs`; the protocol logic is unchanged.
//! Named keys, their text and their escape sequences.
//!
//! The names are GPUI's `Keystroke::key` spellings. The sequences and the
//! Kitty codes come from Alacritty's `input/keyboard.rs` and, for the codes
//! Alacritty does not encode, from the Kitty keyboard protocol. The comments
//! on the tables say which is which.

use super::KeyLocation;
use super::sequence::Terminator;

/// A key with a name instead of a printable character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NamedKey {
    Enter,
    Tab,
    Escape,
    Space,
    Backspace,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    /// Function keys 1 through 35.
    Function(u8),
    Shift,
    Control,
    Alt,
    Super,
    Hyper,
    Meta,
    CapsLock,
    NumLock,
    ScrollLock,
    PrintScreen,
    Pause,
    ContextMenu,
    MediaPlay,
    MediaPause,
    MediaPlayPause,
    MediaStop,
    MediaFastForward,
    MediaRewind,
    MediaTrackNext,
    MediaTrackPrevious,
    MediaRecord,
    AudioVolumeDown,
    AudioVolumeUp,
    AudioVolumeMute,
}

impl NamedKey {
    /// Maps a GPUI key name to a named key.
    ///
    /// GPUI spells names in lowercase, including `pageup`, `pagedown` and
    /// `platform`. An unknown name is a character key, so this returns `None`
    /// rather than guessing.
    pub(super) fn from_name(name: &str) -> Option<Self> {
        let named = match name {
            "enter" | "return" => Self::Enter,
            "tab" => Self::Tab,
            "escape" | "esc" => Self::Escape,
            "space" => Self::Space,
            "backspace" => Self::Backspace,
            "left" | "arrowleft" => Self::ArrowLeft,
            "right" | "arrowright" => Self::ArrowRight,
            "up" | "arrowup" => Self::ArrowUp,
            "down" | "arrowdown" => Self::ArrowDown,
            "home" => Self::Home,
            "end" => Self::End,
            "pageup" => Self::PageUp,
            "pagedown" => Self::PageDown,
            "insert" => Self::Insert,
            "delete" => Self::Delete,
            "shift" => Self::Shift,
            "control" | "ctrl" => Self::Control,
            "alt" | "option" => Self::Alt,
            "platform" | "super" | "win" | "cmd" => Self::Super,
            "hyper" => Self::Hyper,
            "meta" => Self::Meta,
            "capslock" => Self::CapsLock,
            "numlock" => Self::NumLock,
            "scrolllock" => Self::ScrollLock,
            "printscreen" => Self::PrintScreen,
            "pause" => Self::Pause,
            "contextmenu" | "menu" => Self::ContextMenu,
            "mediaplay" => Self::MediaPlay,
            "mediapause" => Self::MediaPause,
            "mediaplaypause" => Self::MediaPlayPause,
            "mediastop" => Self::MediaStop,
            "mediafastforward" => Self::MediaFastForward,
            "mediarewind" => Self::MediaRewind,
            "mediatracknext" => Self::MediaTrackNext,
            "mediatrackprevious" => Self::MediaTrackPrevious,
            "mediarecord" => Self::MediaRecord,
            "audiovolumedown" => Self::AudioVolumeDown,
            "audiovolumeup" => Self::AudioVolumeUp,
            "audiovolumemute" => Self::AudioVolumeMute,
            function if function.starts_with('f') => {
                let number = function.strip_prefix('f')?.parse::<u8>().ok()?;
                (1..=35)
                    .contains(&number)
                    .then_some(Self::Function(number))?
            }
            _ => return None,
        };
        Some(named)
    }

    /// The text a key produces when it is treated as text rather than an
    /// escape sequence.
    ///
    /// This is winit's `NamedKey::to_text`, which Alacritty's text path uses.
    /// Enter is `\r` even though GPUI stores `\n` in `key_char`, and
    /// Backspace is `\x08`; the default binding turns it into `\x7f`.
    pub(super) fn text(self) -> Option<&'static str> {
        match self {
            Self::Enter => Some("\r"),
            Self::Backspace => Some("\x08"),
            Self::Tab => Some("\t"),
            Self::Space => Some(" "),
            Self::Escape => Some("\x1b"),
            _ => None,
        }
    }

    /// Whether the key is a modifier and so should not clear a selection
    /// when it is sent.
    pub(super) fn is_modifier(self) -> bool {
        matches!(
            self,
            Self::Shift | Self::Control | Self::Alt | Self::Super | Self::Hyper | Self::Meta
        )
    }

    /// The application-cursor sequence of a cursor key, if it has one.
    ///
    /// These are Alacritty's `default_key_bindings` for
    /// `+BindingMode::APP_CURSOR` (config/bindings.rs): SS3 cursor keys.
    pub(super) fn application_cursor(self) -> Option<&'static [u8]> {
        match self {
            Self::Home => Some(b"\x1bOH"),
            Self::End => Some(b"\x1bOF"),
            Self::ArrowUp => Some(b"\x1bOA"),
            Self::ArrowDown => Some(b"\x1bOB"),
            Self::ArrowRight => Some(b"\x1bOC"),
            Self::ArrowLeft => Some(b"\x1bOD"),
            _ => None,
        }
    }

    /// The legacy F1 through F4 sequences Alacritty binds when no Kitty mode
    /// is active: `ESC O P` through `ESC O S`.
    pub(super) fn legacy_function_key(self) -> Option<&'static [u8]> {
        match self {
            Self::Function(1) => Some(b"\x1bOP"),
            Self::Function(2) => Some(b"\x1bOQ"),
            Self::Function(3) => Some(b"\x1bOR"),
            Self::Function(4) => Some(b"\x1bOS"),
            _ => None,
        }
    }

    /// The xterm sequence built by Alacritty's `try_build_named_normal`.
    ///
    /// `one_based` is `""` when modifiers, event types and associated text
    /// are all absent, and `"1"` otherwise, mirroring Alacritty's rule that
    /// the default parameter 1 is omitted. Cursor keys in application mode
    /// are handled by [`NamedKey::application_cursor`] first.
    pub(super) fn normal_sequence(self, one_based: &str) -> Option<(String, Terminator)> {
        let normal =
            |payload: &str, terminator| Some((payload.to_string(), Terminator::Normal(terminator)));
        match self {
            Self::PageUp => normal("5", '~'),
            Self::PageDown => normal("6", '~'),
            Self::Insert => normal("2", '~'),
            Self::Delete => normal("3", '~'),
            Self::Home => normal(one_based, 'H'),
            Self::End => normal(one_based, 'F'),
            Self::ArrowLeft => normal(one_based, 'D'),
            Self::ArrowRight => normal(one_based, 'C'),
            Self::ArrowUp => normal(one_based, 'A'),
            Self::ArrowDown => normal(one_based, 'B'),
            Self::Function(1) => normal(one_based, 'P'),
            Self::Function(2) => normal(one_based, 'Q'),
            Self::Function(3) => normal(one_based, 'R'),
            Self::Function(4) => normal(one_based, 'S'),
            Self::Function(5) => normal("15", '~'),
            Self::Function(6) => normal("17", '~'),
            Self::Function(7) => normal("18", '~'),
            Self::Function(8) => normal("19", '~'),
            Self::Function(9) => normal("20", '~'),
            Self::Function(10) => normal("21", '~'),
            Self::Function(11) => normal("23", '~'),
            Self::Function(12) => normal("24", '~'),
            Self::Function(13) => normal("25", '~'),
            Self::Function(14) => normal("26", '~'),
            Self::Function(15) => normal("28", '~'),
            Self::Function(16) => normal("29", '~'),
            Self::Function(17) => normal("31", '~'),
            Self::Function(18) => normal("32", '~'),
            Self::Function(19) => normal("33", '~'),
            Self::Function(20) => normal("34", '~'),
            _ => None,
        }
    }

    /// The Kitty keyboard protocol sequence of a functional key.
    ///
    /// Alacritty's `try_build_named_kitty`, including its note that F3
    /// diverges from its terminfo entry. F13 through F35 and the media keys
    /// are not part of xterm.
    pub(super) fn kitty_sequence(self) -> Option<(&'static str, Terminator)> {
        let (code, terminator) = match self {
            Self::Function(3) => ("13", Terminator::Normal('~')),
            Self::Function(13) => ("57376", Terminator::Kitty),
            Self::Function(14) => ("57377", Terminator::Kitty),
            Self::Function(15) => ("57378", Terminator::Kitty),
            Self::Function(16) => ("57379", Terminator::Kitty),
            Self::Function(17) => ("57380", Terminator::Kitty),
            Self::Function(18) => ("57381", Terminator::Kitty),
            Self::Function(19) => ("57382", Terminator::Kitty),
            Self::Function(20) => ("57383", Terminator::Kitty),
            Self::Function(21) => ("57384", Terminator::Kitty),
            Self::Function(22) => ("57385", Terminator::Kitty),
            Self::Function(23) => ("57386", Terminator::Kitty),
            Self::Function(24) => ("57387", Terminator::Kitty),
            Self::Function(25) => ("57388", Terminator::Kitty),
            Self::Function(26) => ("57389", Terminator::Kitty),
            Self::Function(27) => ("57390", Terminator::Kitty),
            Self::Function(28) => ("57391", Terminator::Kitty),
            Self::Function(29) => ("57392", Terminator::Kitty),
            Self::Function(30) => ("57393", Terminator::Kitty),
            Self::Function(31) => ("57394", Terminator::Kitty),
            Self::Function(32) => ("57395", Terminator::Kitty),
            Self::Function(33) => ("57396", Terminator::Kitty),
            Self::Function(34) => ("57397", Terminator::Kitty),
            Self::Function(35) => ("57398", Terminator::Kitty),
            Self::ScrollLock => ("57359", Terminator::Kitty),
            Self::PrintScreen => ("57361", Terminator::Kitty),
            Self::Pause => ("57362", Terminator::Kitty),
            Self::ContextMenu => ("57363", Terminator::Kitty),
            Self::MediaPlay => ("57428", Terminator::Kitty),
            Self::MediaPause => ("57429", Terminator::Kitty),
            Self::MediaPlayPause => ("57430", Terminator::Kitty),
            Self::MediaStop => ("57432", Terminator::Kitty),
            Self::MediaFastForward => ("57433", Terminator::Kitty),
            Self::MediaRewind => ("57434", Terminator::Kitty),
            Self::MediaTrackNext => ("57435", Terminator::Kitty),
            Self::MediaTrackPrevious => ("57436", Terminator::Kitty),
            Self::MediaRecord => ("57437", Terminator::Kitty),
            Self::AudioVolumeDown => ("57438", Terminator::Kitty),
            Self::AudioVolumeUp => ("57439", Terminator::Kitty),
            Self::AudioVolumeMute => ("57440", Terminator::Kitty),
            _ => return None,
        };
        Some((code, terminator))
    }

    /// The code of a control character in the Kitty protocol.
    ///
    /// Alacritty only encodes these keys when the protocol asks for all keys
    /// or for disambiguation; plain Tab and Enter are left to the text path.
    pub(super) fn control_character_code(self) -> Option<&'static str> {
        match self {
            Self::Tab => Some("9"),
            Self::Enter => Some("13"),
            Self::Escape => Some("27"),
            Self::Space => Some("32"),
            Self::Backspace => Some("127"),
            _ => None,
        }
    }

    /// The Kitty code of a modifier key itself.
    ///
    /// The left and right modifiers have different codes. Alacritty uses the
    /// left code for `KeyLocation::Left` and the generic code otherwise.
    pub(super) fn modifier_code(self, location: KeyLocation) -> Option<&'static str> {
        let left = location == KeyLocation::Left;
        let code = match self {
            Self::Shift if left => "57441",
            Self::Control if left => "57442",
            Self::Alt if left => "57443",
            Self::Super if left => "57444",
            Self::Hyper if left => "57445",
            Self::Meta if left => "57446",
            Self::Shift => "57447",
            Self::Control => "57448",
            Self::Alt => "57449",
            Self::Super => "57450",
            Self::Hyper => "57451",
            Self::Meta => "57452",
            Self::CapsLock => "57358",
            Self::NumLock => "57360",
            _ => return None,
        };
        Some(code)
    }
}
