//! Ranking settings search results, from
//! `src/components/SettingsDialog/settingsSearch.ts`.

use crate::js::{is_js_whitespace, trim_js};

/// A setting, or a whole page, that matches the search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsSearchResult {
    /// The page the result is on.
    pub tab: String,
    /// The page's translated name.
    pub tab_label: String,
    /// The setting's translated name, or the page's own name.
    pub title: String,
    /// The setting's translated description.
    pub description: Option<String>,
    /// The result is the page itself rather than a setting on it.
    pub is_page: bool,
}

/// Normalizes text for comparison.
///
/// The TypeScript version also applies NFKC, which would need a new
/// dependency; full-width input therefore does not match here. See the report
/// for the known difference.
fn normalize(text: &str) -> String {
    trim_js(&text.to_lowercase()).to_owned()
}

/// Ranks how well an item matches.
///
/// Every word of the query has to appear in its title, description or page
/// name, and at least one of them in its own title or description, so a page
/// name alone does not list the whole page.
fn score_item(item: &SettingsSearchResult, query: &str, words: &[&str]) -> u32 {
    let title = normalize(&item.title);
    let own_text = format!(
        "{title} {}",
        normalize(item.description.as_deref().unwrap_or(""))
    );
    let all_text = format!("{own_text} {}", normalize(&item.tab_label));

    if !words.iter().all(|word| all_text.contains(word)) {
        return 0;
    }
    if !words.iter().any(|word| own_text.contains(word)) {
        return 0;
    }
    if title == query {
        return 5;
    }
    if title.starts_with(query) {
        return 4;
    }
    if title.contains(query) {
        return 3;
    }
    if words.iter().all(|word| title.contains(word)) {
        return 2;
    }
    1
}

/// Finds the settings that match the query, best matches first.
pub fn search_settings(items: &[SettingsSearchResult], query: &str) -> Vec<SettingsSearchResult> {
    let normalized = normalize(query);
    if normalized.is_empty() {
        return Vec::new();
    }

    let words: Vec<&str> = normalized
        .split(is_js_whitespace)
        .filter(|word| !word.is_empty())
        .collect();
    let mut matches: Vec<(usize, u32, &SettingsSearchResult)> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let score = score_item(item, &normalized, &words);
            (score > 0).then_some((index, score, item))
        })
        .collect();
    matches.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    matches
        .into_iter()
        .map(|(_, _, item)| item.clone())
        .collect()
}
