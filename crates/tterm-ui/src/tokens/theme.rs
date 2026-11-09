use crate::tokens::palette::Colors;

/// A theme of the stylesheet: the values that `:root` or a `[data-theme="…"]`
/// block declares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// The value of `data-theme` that selects this theme; the default theme is
    /// served from `:root` instead.
    pub id: &'static str,
    /// The colours the theme declares for the interface.
    pub colors: Colors,
}

impl Theme {
    /// Matches the stylesheet's `dark:` variant, active for every theme except
    /// the explicitly light theme.
    pub fn uses_dark_variant(self) -> bool {
        !matches!(self.id, "light")
    }
}
