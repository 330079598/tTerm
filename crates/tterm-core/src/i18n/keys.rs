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
    TAB_NEW => "tabs.newTab",
    TAB_CLOSE => "tabs.closeTab",
    TAB_SCROLL_LEFT => "tabs.scrollLeft",
    TAB_SCROLL_RIGHT => "tabs.scrollRight",
    TAB_SHOW_ALL => "tabs.showAll",
    TAB_SEARCH => "tabs.searchTabs",
    TAB_CLEAR => "tabs.clearSearch",
    TAB_NO_MATCH => "tabs.noMatchingTabs",
    TAB_TERMINAL => "tabs.terminal",
    TAB_SSH => "tabs.ssh",
    SCROLLBACK_SETTING_OPEN => "scrollbackSetting.open",
    SCROLLBACK_SETTING_TITLE => "scrollbackSetting.title",
    SCROLLBACK_SETTING_DESCRIPTION => "scrollbackSetting.description",
    SCROLLBACK_SETTING_PLACEHOLDER => "scrollbackSetting.placeholder",
    SCROLLBACK_SETTING_SAVE => "scrollbackSetting.save",
    SCROLLBACK_SETTING_EMPTY => "scrollbackSetting.empty",
    SCROLLBACK_SETTING_INVALID => "scrollbackSetting.invalid",
    SCROLLBACK_SETTING_TOO_LARGE => "scrollbackSetting.tooLarge",
    PASTE_MULTILINE_TITLE => "terminalPaste.multilineTitle",
    PASTE_MULTILINE_DESCRIPTION => "terminalPaste.multilineDescription",
    PASTE_TRAILING_LINE_BREAK_DESCRIPTION => "terminalPaste.trailingLineBreakDescription",
    PASTE_PASTE_ANYWAY => "terminalPaste.pasteAnyway",
}
