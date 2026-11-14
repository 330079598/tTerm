//! The word search quick connect and the searchable option lists share, from
//! `src/lib/quickConnect.ts`.

use crate::js::is_js_whitespace;

/// Whether every word of `query` appears in one of `fields`, ignoring case.
pub fn matches_query(query: &str, fields: &[Option<&str>]) -> bool {
    let query = query.to_lowercase();
    let words: Vec<&str> = query
        .split(is_js_whitespace)
        .filter(|word| !word.is_empty())
        .collect();

    let mut haystack = String::new();
    for field in fields.iter().flatten().filter(|field| !field.is_empty()) {
        if !haystack.is_empty() {
            haystack.push(' ');
        }
        haystack.push_str(field);
    }
    let haystack = haystack.to_lowercase();

    words.iter().all(|word| haystack.contains(word))
}

#[cfg(test)]
mod tests {
    use super::matches_query;

    #[test]
    fn matches_every_word_in_any_order() {
        assert!(matches_query(
            "night TOMORROW",
            &[Some("Tomorrow Night"), None]
        ));
        assert!(!matches_query("tomorrow day", &[Some("Tomorrow Night")]));
    }

    #[test]
    fn an_empty_query_matches_everything() {
        assert!(matches_query("   ", &[Some("anything")]));
        assert!(matches_query("\u{FEFF}", &[None]));
    }

    #[test]
    fn ignores_missing_and_empty_fields() {
        assert!(matches_query(
            "web hook",
            &[Some("webhook-web"), None, Some("")]
        ));
        assert!(!matches_query("hook", &[None, Some("")]));
    }
}
