//! The settings search over the entries the native page implements.
//!
//! The ranking and the list of pages come from `tterm-core`'s settings
//! search index, shared with the Tauri client. A page is only listable here
//! when its setting is actually rendered; the rest of the shared index stays
//! out until those pages exist.

use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationKey, TranslationRequest};
use tterm_core::settings::search::{SettingsSearchResult, search_settings};
use tterm_core::settings::search_index::entries_for_page;

use crate::locale::Locale;

/// The Terminal page, the route's default.
pub(super) const TERMINAL_PAGE_VALUE: &str = "terminal";
/// The local-shells page.
pub(super) const LOCAL_SHELLS_PAGE_VALUE: &str = "shells";

/// Pages this client implements, in sidebar order.
const PAGES: &[(&str, TranslationKey)] = &[
    (TERMINAL_PAGE_VALUE, keys::SETTINGS_TERMINAL),
    (LOCAL_SHELLS_PAGE_VALUE, keys::LOCAL_SHELLS_SIDEBAR),
];

/// Settings that exist in this client, with the page that owns them.
///
/// Each key must also appear in the shared index; the test below keeps the
/// lists from drifting apart.
const IMPLEMENTED: &[(&str, TranslationKey, Option<TranslationKey>)] = &[
    (
        TERMINAL_PAGE_VALUE,
        keys::FONT_SETTINGS_TEXT_RENDERING,
        Some(keys::FONT_SETTINGS_TEXT_RENDERING_DESC),
    ),
    (TERMINAL_PAGE_VALUE, keys::FONT_SETTINGS_FONT_FAMILY, None),
    (TERMINAL_PAGE_VALUE, keys::FONT_SETTINGS_FONT_SIZE, None),
    (TERMINAL_PAGE_VALUE, keys::FONT_SETTINGS_LINE_HEIGHT, None),
    (
        TERMINAL_PAGE_VALUE,
        keys::FONT_SETTINGS_LETTER_SPACING,
        Some(keys::FONT_SETTINGS_SPACING_DESC),
    ),
    (TERMINAL_PAGE_VALUE, keys::FONT_SETTINGS_CURSOR_STYLE, None),
    (
        TERMINAL_PAGE_VALUE,
        keys::FONT_SETTINGS_SCROLLBACK_LINES,
        None,
    ),
    (
        TERMINAL_PAGE_VALUE,
        keys::FONT_SETTINGS_TERMINAL_PADDING,
        Some(keys::FONT_SETTINGS_TERMINAL_PADDING_DESC),
    ),
    (
        TERMINAL_PAGE_VALUE,
        keys::FONT_SETTINGS_PREVIEW,
        Some(keys::FONT_SETTINGS_PREVIEW_DESC),
    ),
    (
        LOCAL_SHELLS_PAGE_VALUE,
        keys::LOCAL_SHELLS_TITLE,
        Some(keys::LOCAL_SHELLS_DESCRIPTION),
    ),
    (
        LOCAL_SHELLS_PAGE_VALUE,
        keys::LOCAL_SHELLS_LIST_TITLE,
        Some(keys::LOCAL_SHELLS_LIST_DESCRIPTION),
    ),
];

/// The pages and settings a search may list.
pub(super) fn items(locale: &Locale) -> Vec<SettingsSearchResult> {
    let mut items = Vec::new();
    for (page, label) in PAGES {
        let label = text(locale, *label);
        items.push(SettingsSearchResult {
            tab: (*page).to_string(),
            tab_label: label.clone(),
            title: label,
            description: None,
            is_page: true,
        });
    }
    // The shared index decides which page owns an entry and in what order.
    // Only the entries that this client implements survive the filter.
    for (page, title, description) in IMPLEMENTED {
        if !entries_for_page(page)
            .iter()
            .any(|entry| entry.title == title.as_str())
        {
            continue;
        }
        let Some(tab_label) = items
            .iter()
            .find(|item| item.is_page && item.tab == *page)
            .map(|item| item.tab_label.clone())
        else {
            continue;
        };
        items.push(SettingsSearchResult {
            tab: (*page).to_string(),
            tab_label,
            title: text(locale, *title),
            description: description.map(|key| text(locale, key)),
            is_page: false,
        });
    }
    items
}

/// Ranks the implemented settings for a query.
pub(super) fn results(locale: &Locale, query: &str) -> Vec<SettingsSearchResult> {
    search_settings(&items(locale), query)
}

/// The translated result count.
pub(super) fn count_text(locale: &Locale, count: usize) -> String {
    let request = TranslationRequest::new(keys::SETTINGS_SEARCH_COUNT).count(count as f64);
    locale.translate(request).unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to translate the settings result count: {error}");
        count.to_string()
    })
}

/// The translated no-match message with the query filled in.
pub(super) fn empty_text(locale: &Locale, query: &str) -> String {
    let query = query.trim();
    let arguments = [("query", ArgumentValue::Text(query))];
    let request = TranslationRequest::new(keys::SETTINGS_SEARCH_EMPTY).arguments(&arguments);
    locale.translate(request).unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to translate the empty settings search: {error}");
        keys::SETTINGS_SEARCH_EMPTY.as_str().to_string()
    })
}

pub(super) fn text(locale: &Locale, key: TranslationKey) -> String {
    locale.text(key).unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to translate a settings label: {error}");
        key.as_str().to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tterm_core::settings::search_index::SETTINGS_PAGES;

    #[test]
    fn every_searchable_setting_exists_in_the_shared_index() {
        for (page, title, _) in IMPLEMENTED {
            assert!(
                entries_for_page(page)
                    .iter()
                    .any(|entry| entry.title == title.as_str()),
                "{} is not in the shared settings index",
                title.as_str()
            );
        }
    }

    #[test]
    fn every_page_is_in_the_shared_sidebar() {
        for (value, label_key) in PAGES {
            let page = SETTINGS_PAGES
                .iter()
                .find(|page| page.value == *value)
                .expect("the implemented page is missing from the shared sidebar");
            assert_eq!(page.label_key, label_key.as_str());
        }
    }
}
