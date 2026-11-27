use crate::tokens::palette::Colors;

/// A theme of the stylesheet: the values that `:root` or a `[data-theme="…"]`
/// block declares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// The value of `data-theme` that selects this theme; the default theme is
    /// served from `:root` instead. A theme from the bundled library keeps its
    /// own id here, which is not a selector the stylesheet declares.
    pub id: &'static str,
    /// The colours the theme declares for the interface.
    pub colors: Colors,
    /// Whether the stylesheet's dark variant applies. This is kept separately
    /// from `id` because a library theme's id does not name a preset selector.
    pub(crate) dark: bool,
}

impl Theme {
    /// A theme built from a bundled library entry: the entry's own id, the
    /// interface colours derived from it, and the darkness that derivation
    /// decided rather than one guessed from the id.
    pub const fn library(id: &'static str, colors: Colors, dark: bool) -> Self {
        Self { id, colors, dark }
    }

    /// Matches the stylesheet's `dark:` variant.
    pub fn uses_dark_variant(self) -> bool {
        self.dark
    }
}
