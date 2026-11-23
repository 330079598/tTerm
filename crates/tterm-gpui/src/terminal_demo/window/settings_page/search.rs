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

/// The one settings page the native client implements today.
pub(super) const PAGE_VALUE: &str = "terminal";

/// Settings that exist in this client, in page order.
///
/// Each key must also appear in the shared index; the test below keeps the
/// two lists from drifting apart.
const IMPLEMENTED: &[(TranslationKey, Option<TranslationKey>)] =
    &[(keys::FONT_SETTINGS_SCROLLBACK_LINES, None)];

/// The page and settings a search may list.
pub(super) fn items(locale: &Locale) -> Vec<SettingsSearchResult> {
    let tab_label = text(locale, keys::SETTINGS_TERMINAL);
    let mut items = vec![SettingsSearchResult {
        tab: PAGE_VALUE.to_string(),
        tab_label: tab_label.clone(),
        title: tab_label,
        description: None,
        is_page: true,
    }];
    // The shared index decides which page owns an entry and in what order.
    // Only the entries that this client implements survive the filter.
    for entry in entries_for_page(PAGE_VALUE) {
        let Some((title, description)) = IMPLEMENTED
            .iter()
            .find(|(key, _)| key.as_str() == entry.title)
        else {
            continue;
        };
        items.push(SettingsSearchResult {
            tab: PAGE_VALUE.to_string(),
            tab_label: items[0].tab_label.clone(),
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
        for (title, _) in IMPLEMENTED {
            assert!(
                entries_for_page(PAGE_VALUE)
                    .iter()
                    .any(|entry| entry.title == title.as_str()),
                "{} is not in the shared settings index",
                title.as_str()
            );
        }
        let page = SETTINGS_PAGES
            .iter()
            .find(|page| page.value == PAGE_VALUE)
            .expect("the implemented page is missing from the shared sidebar");
        assert_eq!(page.label_key, keys::SETTINGS_TERMINAL.as_str());
    }
}
