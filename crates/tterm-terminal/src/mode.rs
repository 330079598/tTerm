//! The terminal modes that change how the view routes input and what it
//! draws.
//!
//! The flags are Alacritty's, so converting is a newtype wrap; only the names
//! belong to tTerm. A test compares every constant with the pinned crate's
//! flag, so upgrading Alacritty cannot silently renumber a mode that the view
//! and the input encoder rely on.

use alacritty_terminal::term::TermMode;

/// A set of terminal mode flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modes(u32);

impl Modes {
    /// No mode is set.
    pub const NONE: Self = Self(0);
    /// The cursor is visible (DECTCEM).
    pub const SHOW_CURSOR: Self = Self(1);
    /// Cursor keys send the application sequences (DECCKM).
    pub const APP_CURSOR: Self = Self(1 << 1);
    /// The numeric keypad sends application sequences (DECKPAM).
    pub const APP_KEYPAD: Self = Self(1 << 2);
    /// The program wants button presses and releases (mode 1000).
    pub const MOUSE_REPORT_CLICK: Self = Self(1 << 3);
    /// Pasted text is wrapped in the bracketed-paste sequence (mode 2004).
    pub const BRACKETED_PASTE: Self = Self(1 << 4);
    /// Mouse reports use the SGR encoding (mode 1006).
    pub const SGR_MOUSE: Self = Self(1 << 5);
    /// The program wants motion reports while a button is held (mode 1002).
    pub const MOUSE_MOTION: Self = Self(1 << 6);
    /// A character at the last column wraps to the next line (DECAWM).
    pub const LINE_WRAP: Self = Self(1 << 7);
    /// A line feed also returns the carriage (LNM).
    pub const LINE_FEED_NEW_LINE: Self = Self(1 << 8);
    /// Cursor addressing is relative to the scroll region (DECOM).
    pub const ORIGIN: Self = Self(1 << 9);
    /// New characters shift the rest of the line right (IRM).
    pub const INSERT: Self = Self(1 << 10);
    /// The program wants focus in and focus out reports (mode 1004).
    pub const FOCUS_IN_OUT: Self = Self(1 << 11);
    /// The alternate screen is active (mode 1049 and relatives).
    pub const ALT_SCREEN: Self = Self(1 << 12);
    /// The program wants motion reports while a button is held and dragged
    /// (mode 1003).
    pub const MOUSE_DRAG: Self = Self(1 << 13);
    /// Mouse coordinates are encoded as UTF-8 (mode 1005).
    pub const UTF8_MOUSE: Self = Self(1 << 14);
    /// The wheel sends arrow keys on the alternate screen (mode 1007).
    pub const ALTERNATE_SCROLL: Self = Self(1 << 15);
    /// Vi mode is active.
    pub const VI: Self = Self(1 << 16);
    /// The program asks for urgency hints.
    pub const URGENCY_HINTS: Self = Self(1 << 17);
    /// The Kitty keyboard protocol disambiguates escape codes.
    pub const DISAMBIGUATE_ESC_CODES: Self = Self(1 << 18);
    /// The Kitty keyboard protocol reports press, repeat and release events.
    pub const REPORT_EVENT_TYPES: Self = Self(1 << 19);
    /// The Kitty keyboard protocol reports the shifted keys as alternate keys.
    pub const REPORT_ALTERNATE_KEYS: Self = Self(1 << 20);
    /// The Kitty keyboard protocol reports every key as an escape code.
    pub const REPORT_ALL_KEYS_AS_ESC: Self = Self(1 << 21);
    /// The Kitty keyboard protocol reports the text associated with a key.
    pub const REPORT_ASSOCIATED_TEXT: Self = Self(1 << 22);
    /// The union of the mouse-reporting modes.
    pub const MOUSE_MODE: Self =
        Self(Self::MOUSE_REPORT_CLICK.0 | Self::MOUSE_MOTION.0 | Self::MOUSE_DRAG.0);

    /// Whether every flag of `other` is set in `self`.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any flag of `other` is set in `self`.
    ///
    /// Views use it to ask "is mouse reporting on at all" against
    /// [`Modes::MOUSE_MODE`] without naming each of the three modes.
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// The raw flags, for logging and for the test that pins them.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl From<TermMode> for Modes {
    fn from(mode: TermMode) -> Self {
        Self(mode.bits())
    }
}
