//! The rows that changed since the previous snapshot.
//!
//! Alacritty tracks damage per screen row, but only in-place updates use it:
//! every time the scroll region scrolls — each newline once output reaches the
//! bottom — the whole screen is marked damaged, and so is every change of the
//! display offset, including the one that keeps a scrolled-back viewport on
//! the same history when output arrives (design note 4.3). A `Full` damage therefore
//! says "copy and draw everything"; per-row damage only saves work for output
//! that rewrites the screen in place, such as a prompt, vim or a progress bar.
//!
//! Two consequences are relied on by the rest of tTerm:
//!
//! - The row under the cursor is part of the damaged rows in almost every
//!   frame, because Alacritty damages the old and the new cursor position. A
//!   damaged row does not mean its characters changed.
//! - Damage is consumed when a snapshot is taken and reset afterwards, so the
//!   next snapshot only reports changes after that frame.

use alacritty_terminal::term::{Term, TermDamage};

/// A screen row whose cells may have changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DamagedLine {
    /// The screen row, zero-based from the top of the viewport.
    pub line: usize,
    /// The first column that may have changed.
    pub left: usize,
    /// The last column that may have changed, inclusive.
    pub right: usize,
}

/// What changed in the visible area since the previous snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Damage {
    /// The whole screen may have changed; nothing can be reused.
    Full,
    /// Only these rows may have changed, in ascending screen order.
    Rows(Vec<DamagedLine>),
}

impl Damage {
    /// Whether the whole screen has to be redrawn.
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }

    /// The changed rows, sorted by screen row.
    ///
    /// This is empty for [`Damage::Full`]: ask [`Damage::is_full`] first, or
    /// treat an empty list as "no row information", not as "nothing changed".
    pub fn rows(&self) -> &[DamagedLine] {
        match self {
            Self::Full => &[],
            Self::Rows(rows) => rows,
        }
    }

    /// Consumes Alacritty's damage and resets it, so the next snapshot only
    /// reports what changed afterwards.
    pub(crate) fn take<T>(term: &mut Term<T>) -> Self {
        let damage = match term.damage() {
            TermDamage::Full => Self::Full,
            TermDamage::Partial(rows) => Self::Rows(
                rows.map(|row| DamagedLine {
                    line: row.line,
                    left: row.left,
                    right: row.right,
                })
                .collect(),
            ),
        };
        term.reset_damage();
        damage
    }
}
