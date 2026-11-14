//! Searching saved connections, from `src/lib/profileSearch.ts`.

use crate::js::{is_js_whitespace, trim_js};
use crate::profiles::types::SavedProfile;
use crate::text_search::matches_query;
use std::collections::HashMap;

/// The fields a search for saved connections looks in.
pub fn profile_search_fields(profile: &SavedProfile) -> Vec<Option<&str>> {
    vec![
        Some(profile.name.as_str()),
        profile.group_path.as_deref(),
        profile.host.as_deref(),
        profile.username.as_deref(),
        profile.serial.as_ref().map(|serial| serial.port.as_str()),
        profile
            .local_terminal
            .as_ref()
            .and_then(|local| local.start_directory.as_deref()),
    ]
}

fn normalized_phrase(query: &str) -> String {
    let mut words = Vec::new();
    for word in trim_js(query).split(is_js_whitespace) {
        if !word.is_empty() {
            words.push(word);
        }
    }
    words.join(" ").to_lowercase()
}

fn name_words(name: &str) -> impl Iterator<Item = &str> {
    name.split(|character| is_js_whitespace(character) || matches!(character, '-' | '_' | '.'))
}

/// How well a profile matches a search, higher first.
pub fn profile_match_score(profile: &SavedProfile, query: &str) -> u32 {
    let phrase = normalized_phrase(query);
    if phrase.is_empty() {
        return 0;
    }
    let name = profile.name.to_lowercase();
    if name == phrase {
        return 4;
    }
    if name.starts_with(&phrase) || name_words(&name).any(|word| word.starts_with(&phrase)) {
        return 3;
    }
    if phrase.split(' ').all(|word| name.contains(word))
        || profile
            .host
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .starts_with(&phrase)
    {
        return 2;
    }
    1
}

/// Profiles that match a search, best fit first; equal fits keep the saved
/// order after the one used last.
pub fn rank_profiles(
    profiles: &[SavedProfile],
    query: &str,
    last_used: &HashMap<String, i64>,
) -> Vec<SavedProfile> {
    let scores: HashMap<String, u32> = profiles
        .iter()
        .map(|profile| (profile.id.clone(), profile_match_score(profile, query)))
        .collect();
    let mut ranked = profiles.to_vec();
    ranked.sort_by(|left, right| {
        let score_left = scores.get(&left.id).copied().unwrap_or(0);
        let score_right = scores.get(&right.id).copied().unwrap_or(0);
        let use_left = last_used.get(&left.id).copied().unwrap_or(0);
        let use_right = last_used.get(&right.id).copied().unwrap_or(0);
        score_right
            .cmp(&score_left)
            .then_with(|| use_right.cmp(&use_left))
    });
    ranked
}

/// Whether every word of `query` appears in the profile's searchable fields.
pub fn profile_matches_query(profile: &SavedProfile, query: &str) -> bool {
    matches_query(query, &profile_search_fields(profile))
}

#[cfg(test)]
mod tests {
    use super::{profile_match_score, profile_matches_query, rank_profiles};
    use crate::profiles::types::SavedProfile;
    use crate::serial::settings::default_serial_options;
    use std::collections::HashMap;

    fn profile(id: &str, name: &str) -> SavedProfile {
        let mut profile: SavedProfile = serde_json::from_value(serde_json::json!({
            "id": id,
            "name": name,
            "group_path": "Production",
            "connection_type": "ssh",
            "host": "10.0.0.5",
            "port": 22,
            "username": "root",
            "remember_password": true,
            "keepalive_interval_secs": 15,
            "keepalive_count_max": 3
        }))
        .expect("profile");
        if id != "p1" {
            profile.host = None;
            profile.username = None;
            profile.group_path = None;
        }
        profile
    }

    #[test]
    fn finds_a_connection_by_words_from_different_fields_in_any_order() {
        let saved = profile("p1", "web-01");
        assert!(profile_matches_query(&saved, "root web"));
        assert!(profile_matches_query(&saved, "prod 10.0.0"));
        assert!(!profile_matches_query(&saved, "web admin"));
    }

    #[test]
    fn finds_a_serial_connection_by_its_port() {
        let mut saved = profile("p1", "board");
        saved.host = None;
        saved.username = None;
        saved.serial = Some(default_serial_options());
        if let Some(serial) = &mut saved.serial {
            serial.port = "COM3".to_owned();
        }
        assert!(profile_matches_query(&saved, "com3"));
    }

    #[test]
    fn scores_the_name_above_a_word_start_above_other_fields() {
        let score = |name: &str, host: Option<&str>, query: &str| {
            let mut saved = profile("p1", name);
            saved.host = host.map(str::to_owned);
            profile_match_score(&saved, query)
        };
        assert_eq!(score("web", None, "WEB"), 4);
        assert_eq!(score("web-01", None, "web"), 3);
        assert_eq!(score("prod-web-01", None, "web"), 3);
        assert_eq!(score("webhook-web", None, "hook web"), 2);
        assert_eq!(score("gateway", Some("10.0.0.5"), "10.0"), 2);
        assert_eq!(score("gateway", None, "root"), 1);
        assert_eq!(score("web-01", None, "  "), 0);
    }

    #[test]
    fn ranks_best_fit_then_last_use_then_saved_order() {
        let profiles = vec![
            {
                let mut saved = profile("user", "gateway");
                saved.username = Some("web".to_owned());
                saved
            },
            {
                let mut saved = profile("older", "web-02");
                saved.username = None;
                saved
            },
            {
                let mut saved = profile("newer", "web-03");
                saved.username = None;
                saved
            },
            {
                let mut saved = profile("exact", "web");
                saved.username = None;
                saved
            },
            {
                let mut saved = profile("unused", "web-04");
                saved.username = None;
                saved
            },
        ];
        let last_used = HashMap::from([("older".to_owned(), 1), ("newer".to_owned(), 2)]);
        assert_eq!(
            rank_profiles(&profiles, "web", &last_used)
                .iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>(),
            vec!["exact", "newer", "older", "unused", "user"]
        );
    }

    #[test]
    fn orders_by_last_use_alone_with_nothing_typed() {
        let profiles = ["a", "b", "c"].map(|id| profile(id, "web")).to_vec();
        let last_used = HashMap::from([("c".to_owned(), 5)]);
        assert_eq!(
            rank_profiles(&profiles, "", &last_used)
                .iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>(),
            vec!["c", "a", "b"]
        );
    }
}
