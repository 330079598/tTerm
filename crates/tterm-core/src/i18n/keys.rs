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
    LAYOUT_TEMPLATES_OPEN => "layoutTemplates.menu",
    LAYOUT_TEMPLATES_TITLE => "layoutTemplates.title",
    LAYOUT_TEMPLATES_DESCRIPTION => "layoutTemplates.description",
    LAYOUT_TEMPLATES_BUILTIN => "layoutTemplates.builtin",
    LAYOUT_TEMPLATES_BUILTIN_COLUMNS_2 => "layoutTemplates.builtins.columns2",
    LAYOUT_TEMPLATES_BUILTIN_ROWS_2 => "layoutTemplates.builtins.rows2",
    LAYOUT_TEMPLATES_BUILTIN_COLUMNS_3 => "layoutTemplates.builtins.columns3",
    LAYOUT_TEMPLATES_BUILTIN_MAIN_RIGHT_2 => "layoutTemplates.builtins.mainRight2",
    LAYOUT_TEMPLATES_BUILTIN_MAIN_BOTTOM_2 => "layoutTemplates.builtins.mainBottom2",
    LAYOUT_TEMPLATES_BUILTIN_GRID_4 => "layoutTemplates.builtins.grid4",
    LAYOUT_TEMPLATES_SAVED => "layoutTemplates.saved",
    LAYOUT_TEMPLATES_NONE_SAVED => "layoutTemplates.noneSaved",
    LAYOUT_TEMPLATES_SAVE_CURRENT => "layoutTemplates.saveCurrent",
    LAYOUT_TEMPLATES_SPLIT_FIRST => "layoutTemplates.splitFirst",
    LAYOUT_TEMPLATES_NAME => "layoutTemplates.name",
    LAYOUT_TEMPLATES_SAVE => "layoutTemplates.save",
    LAYOUT_TEMPLATES_UNTITLED => "layoutTemplates.untitled",
    LAYOUT_TEMPLATES_PANE_COUNT => "layoutTemplates.paneCount",
    LAYOUT_TEMPLATES_NEEDS_SESSION => "layoutTemplates.needsSession",
    LAYOUT_TEMPLATES_DELETE => "layoutTemplates.delete",
    AGENT_RESUME_CONTINUE => "agentResume.continue",
    AGENT_RESUME_TIMEOUT => "agentResume.timeout",
    AGENT_RESUME_UNKNOWN_SHELL => "agentResume.unknownShell",
    AGENT_RESUME_UNAVAILABLE => "agentResume.unavailable",
    AGENT_RESUME_UNSAFE => "agentResume.unsafe",
    AGENT_RESUME_FAILED => "agentResume.failed",
    PASTE_MULTILINE_TITLE => "terminalPaste.multilineTitle",
    PASTE_MULTILINE_DESCRIPTION => "terminalPaste.multilineDescription",
    PASTE_TRAILING_LINE_BREAK_DESCRIPTION => "terminalPaste.trailingLineBreakDescription",
    PASTE_PASTE_ANYWAY => "terminalPaste.pasteAnyway",
}
