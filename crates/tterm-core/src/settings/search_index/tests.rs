//! Data tests transcribed from `settingsSearch.test.ts`.

use std::collections::HashSet;

use serde_json::Value;

use crate::settings::search_index::{SETTINGS_PAGES, SETTINGS_SEARCH_INDEX, entries_for_page};

fn messages(source: &str) -> Value {
    serde_json::from_str(source).expect("valid i18n JSON")
}

fn lookup<'a>(messages: &'a Value, key: &str) -> Option<&'a str> {
    key.split('.')
        .try_fold(messages, |node, part| node.get(part))?
        .as_str()
}

#[test]
fn lists_only_pages_the_sidebar_has() {
    let pages: HashSet<&str> = SETTINGS_PAGES.iter().map(|page| page.value).collect();
    assert_eq!(pages.len(), SETTINGS_PAGES.len());
    assert!(
        SETTINGS_SEARCH_INDEX
            .iter()
            .all(|index| pages.contains(index.page))
    );
}

#[test]
fn names_only_keys_both_languages_translate() {
    let english = messages(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../src/i18n/locales/en.json"
    )));
    let chinese = messages(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../src/i18n/locales/zh.json"
    )));
    for entry in SETTINGS_SEARCH_INDEX.iter().flat_map(|index| index.entries) {
        for key in [entry.title, entry.description.unwrap_or(entry.title)] {
            assert!(
                lookup(&english, key).is_some(),
                "{key} is missing from en.json"
            );
            assert!(
                lookup(&chinese, key).is_some(),
                "{key} is missing from zh.json"
            );
        }
    }
}

#[test]
fn returns_the_entries_for_the_page_and_none_for_an_unknown_page() {
    assert_eq!(
        entries_for_page("updates")
            .iter()
            .map(|entry| entry.title)
            .collect::<Vec<_>>(),
        [
            "updates.channel",
            "updates.autoDownload",
            "updates.checkFrequency",
            "updates.manualCheck"
        ]
    );
    assert!(entries_for_page("missing").is_empty());
}
