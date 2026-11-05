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
