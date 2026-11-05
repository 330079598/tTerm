//! One cell of the visible grid, in the form the drawing layer reads it.
//!
//! Alacritty's cell type is copied into an owned value while the lock is
//! held; after that the view owns plain data with no lifetime tied to the
//! terminal. Wide characters keep their extra flags so the view can skip the
//! placeholder column, and a zero-width cluster keeps its combining marks.

use std::sync::Arc;

use alacritty_terminal::term::cell::{Cell as AnsiCell, Flags};

use crate::color::Color;

/// The underline decoration of a cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Underline {
    /// No underline.
    #[default]
    None,
    /// A single line under the text.
    Single,
    /// Two lines under the text.
    Double,
    /// A wavy line under the text.
    Curly,
    /// A dotted line under the text.
    Dotted,
    /// A dashed line under the text.
    Dashed,
}

/// The visual attributes of a cell apart from its colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Attributes {
    /// Bold text.
    pub bold: bool,
    /// Dimmed text.
    pub dim: bool,
    /// Italic text.
    pub italic: bool,
    /// Swapped foreground and background.
    pub inverse: bool,
    /// Invisible text, which still occupies its cells.
    pub hidden: bool,
    /// Struck-through text.
    pub strikeout: bool,
    /// The underline decoration.
    pub underline: Underline,
    /// The line continues on the next row without a newline.
    pub wrapline: bool,
    /// The first column of a wide character.
    pub wide: bool,
    /// The second column of a wide character; it draws nothing itself.
    pub wide_spacer: bool,
    /// A wide character wrapped so that only its placeholder fits on this
    /// line; the character itself is at the end of the previous line.
    pub leading_wide_spacer: bool,
}

impl Attributes {
    fn from_flags(flags: Flags) -> Self {
        Self {
            bold: flags.contains(Flags::BOLD),
            dim: flags.contains(Flags::DIM),
            italic: flags.contains(Flags::ITALIC),
            inverse: flags.contains(Flags::INVERSE),
            hidden: flags.contains(Flags::HIDDEN),
            strikeout: flags.contains(Flags::STRIKEOUT),
            underline: underline_from_flags(flags),
            wrapline: flags.contains(Flags::WRAPLINE),
            wide: flags.contains(Flags::WIDE_CHAR),
            wide_spacer: flags.contains(Flags::WIDE_CHAR_SPACER),
            leading_wide_spacer: flags.contains(Flags::LEADING_WIDE_CHAR_SPACER),
        }
    }
}

/// SGR 4:x sets exactly one underline style, so the order only matters if a
/// program sets several bits itself.
fn underline_from_flags(flags: Flags) -> Underline {
    if flags.contains(Flags::DOUBLE_UNDERLINE) {
        Underline::Double
    } else if flags.contains(Flags::UNDERCURL) {
        Underline::Curly
    } else if flags.contains(Flags::DOTTED_UNDERLINE) {
        Underline::Dotted
    } else if flags.contains(Flags::DASHED_UNDERLINE) {
        Underline::Dashed
    } else if flags.contains(Flags::UNDERLINE) {
        Underline::Single
    } else {
        Underline::None
    }
}

/// An OSC 8 hyperlink covering one or more cells.
///
/// The identifier and URI are shared through [`Arc`] because one link usually
/// covers a whole line's worth of cells; cloning a cell then only bumps a
/// reference count.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Hyperlink {
    id: Arc<str>,
    uri: Arc<str>,
}

impl Hyperlink {
    /// Builds a link from what Alacritty stored with the cell.
    pub(crate) fn new(id: &str, uri: &str) -> Self {
        Self {
            id: Arc::from(id),
            uri: Arc::from(uri),
        }
    }

    /// The identifier the program gave the link, or one Alacritty generated.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The URI the link points at.
    pub fn uri(&self) -> &str {
        &self.uri
    }
}

/// One cell of a snapshot row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The character in the cell, or a space when it is empty.
    pub ch: char,
    /// The foreground color, in the form the program asked for.
    pub foreground: Color,
    /// The background color, in the form the program asked for.
    pub background: Color,
    /// Bold, underline and the other visual attributes.
    pub attributes: Attributes,
    /// The underline color when the program set one apart from the text.
    pub underline_color: Option<Color>,
    /// The hyperlink covering the cell, if the program set one.
    pub hyperlink: Option<Hyperlink>,
    /// Grapheme extenders drawn after the cell's character, such as a
    /// combining accent or a variation selector.
    pub zerowidth: Option<Vec<char>>,
}

impl Cell {
    /// Copies one cell out of Alacritty's grid.
    pub(crate) fn from_ansi(cell: &AnsiCell) -> Self {
        Self {
            ch: cell.c,
            foreground: cell.fg.into(),
            background: cell.bg.into(),
            attributes: Attributes::from_flags(cell.flags),
            underline_color: cell.underline_color().map(Into::into),
            hyperlink: cell
                .hyperlink()
                .map(|link| Hyperlink::new(link.id(), link.uri())),
            zerowidth: cell.zerowidth().map(<[char]>::to_vec),
        }
    }

    /// Whether the cell draws no character at all: a space and no combining
    /// mark. It can still have a background color.
    pub fn is_blank(&self) -> bool {
        matches!(self.ch, ' ' | '\t') && self.zerowidth.is_none()
    }
}
