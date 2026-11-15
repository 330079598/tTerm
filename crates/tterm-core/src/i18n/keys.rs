//! Catalogue keys referenced by the GPUI client.
//!
//! The macro below adds every declared key to [`ALL`], so the catalogue tests
//! cannot miss a production reference that was added to this registry.

use super::translate::TranslationKey;

macro_rules! translation_keys {
    ($($name:ident => $key:literal),+ $(,)?) => {
        $(
            #[doc = concat!("The catalogue key `", $key, "`.")]
            pub const $name: TranslationKey = TranslationKey::new($key);
        )+
        /// Every key the GPUI client references.
        pub const ALL: &[TranslationKey] = &[$($name),+];
    };
}

translation_keys! {
    APP_TITLE => "app.title",
    APP_SUBTITLE => "app.subtitle",
    COMMON_CANCEL => "common.cancel",
    PASTE_MULTILINE_TITLE => "terminalPaste.multilineTitle",
    PASTE_MULTILINE_DESCRIPTION => "terminalPaste.multilineDescription",
    PASTE_TRAILING_LINE_BREAK_DESCRIPTION => "terminalPaste.trailingLineBreakDescription",
    PASTE_PASTE_ANYWAY => "terminalPaste.pasteAnyway",
}
