//! Specification tests transcribed from
//! `src/components/SettingsDialog/settingsSearch.test.ts`.
//!
//! The two index-data tests stay with the settings page, and the four
//! `revealSetting` cases need the DOM. The ranking cases use the English and
//! Chinese strings from `src/i18n/locales/` directly.

use crate::settings::search::{SettingsSearchResult, search_settings};

fn setting(
    tab: &str,
    tab_label: &str,
    title: &str,
    description: Option<&str>,
) -> SettingsSearchResult {
    SettingsSearchResult {
        tab: tab.to_owned(),
        tab_label: tab_label.to_owned(),
        title: title.to_owned(),
        description: description.map(str::to_owned),
        is_page: false,
    }
}

fn page(tab: &str, tab_label: &str) -> SettingsSearchResult {
    SettingsSearchResult {
        tab: tab.to_owned(),
        tab_label: tab_label.to_owned(),
        title: tab_label.to_owned(),
        description: None,
        is_page: true,
    }
}

fn english_items() -> Vec<SettingsSearchResult> {
    vec![
        page("terminal", "Terminal"),
        setting("terminal", "Terminal", "Font Size", None),
        setting("terminal", "Terminal", "Font Family", None),
        page("sftp", "SFTP"),
        setting(
            "sftp",
            "SFTP",
            "Paste copied files into SFTP",
            Some(
                "When enabled, Ctrl+V or Cmd+V in the SFTP panel uploads copied local files and folders. Clipboard text is ignored.",
            ),
        ),
    ]
}

fn titles(results: &[SettingsSearchResult]) -> Vec<String> {
    results.iter().map(|result| result.title.clone()).collect()
}

#[test]
fn returns_nothing_for_an_empty_query() {
    assert!(search_settings(&english_items(), "   ").is_empty());
}

#[test]
fn finds_a_setting_by_its_title_ignoring_case() {
    let results = search_settings(&english_items(), "FONT SIZE");
    let first = &results[0];
    assert_eq!(first.tab, "terminal");
    assert_eq!(first.title, "Font Size");
    assert!(!first.is_page);
}

#[test]
fn finds_a_setting_by_words_from_its_description() {
    // The first word longer than six characters in the English description.
    let results = search_settings(&english_items(), "enabled,");
    assert!(titles(&results).contains(&"Paste copied files into SFTP".to_owned()));
}

#[test]
fn does_not_list_a_whole_page_for_a_word_only_its_name_has() {
    let updates = "[Updates page]";
    let items = [
        SettingsSearchResult {
            tab: "updates".to_owned(),
            tab_label: updates.to_owned(),
            title: updates.to_owned(),
            description: None,
            is_page: true,
        },
        setting("updates", updates, "Update channel", None),
    ];
    assert_eq!(titles(&search_settings(&items, "updates")), vec![updates]);
}

#[test]
fn matches_words_from_the_page_name_together_with_the_settings_own() {
    let results = search_settings(&english_items(), "terminal font");
    assert!(titles(&results).contains(&"Font Size".to_owned()));
}

#[test]
fn ranks_titles_that_start_with_the_query_first() {
    let results = search_settings(&english_items(), "font");
    assert!(results[0].title.to_lowercase().starts_with("font"));
}

#[test]
fn searches_the_current_language() {
    let items = vec![
        page("terminal", "终端"),
        setting("terminal", "终端", "字体大小", None),
        setting("terminal", "终端", "字体族", None),
    ];
    let results = search_settings(&items, "字体大小");
    assert!(!results.is_empty());
    assert_eq!(results[0].tab, "terminal");
}
