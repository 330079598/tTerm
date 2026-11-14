//! Recently used saved connections, from `src/lib/profileRecents.ts`.
//!
//! The WebView still owns the `localStorage` read and write; these functions
//! take the stored text and return the value to persist.

use serde_json::Value;

/// When a saved connection last reached a shell or opened its port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentProfile {
    /// The saved profile's id.
    pub profile_id: String,
    /// Unix milliseconds of the last use.
    pub last_used_at: i64,
}

/// The `localStorage` key the WebView keeps the list under.
pub const PROFILE_RECENTS_STORAGE_KEY: &str = "tterm.profile-recents.v1";

/// More than the launcher shows, since a search also puts the saved
/// connections used lately first.
pub const PROFILE_RECENTS_LIMIT: usize = 50;

/// Moves the profile to the front, newest first.
pub fn add_recent_profile(
    recents: &[RecentProfile],
    profile_id: &str,
    used_at: i64,
) -> Vec<RecentProfile> {
    let mut next = vec![RecentProfile {
        profile_id: profile_id.to_owned(),
        last_used_at: used_at,
    }];
    next.extend(
        recents
            .iter()
            .filter(|recent| recent.profile_id != profile_id)
            .cloned(),
    );
    next.truncate(PROFILE_RECENTS_LIMIT);
    next
}

/// Forgets a profile, which removes it from the launcher's recent list.
pub fn remove_recent_profile(recents: &[RecentProfile], profile_id: &str) -> Vec<RecentProfile> {
    recents
        .iter()
        .filter(|recent| recent.profile_id != profile_id)
        .cloned()
        .collect()
}

/// When each profile was last used, for sorting.
pub fn profile_last_used(recents: &[RecentProfile]) -> std::collections::HashMap<String, i64> {
    recents
        .iter()
        .map(|recent| (recent.profile_id.clone(), recent.last_used_at))
        .collect()
}

/// Reads the stored list, keeping only the well-formed entries.
pub fn parse_recent_profiles(raw: Option<&str>) -> Vec<RecentProfile> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    let Value::Array(items) = parsed else {
        return Vec::new();
    };

    items
        .into_iter()
        .filter_map(|item| {
            let Value::Object(object) = item else {
                return None;
            };
            let profile_id = object.get("profileId")?.as_str()?.to_owned();
            let last_used_at = object.get("lastUsedAt")?.as_i64()?;
            Some(RecentProfile {
                profile_id,
                last_used_at,
            })
        })
        .take(PROFILE_RECENTS_LIMIT)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        PROFILE_RECENTS_LIMIT, RecentProfile, add_recent_profile, parse_recent_profiles,
        remove_recent_profile,
    };

    #[test]
    fn moves_a_profile_used_again_to_the_front() {
        let mut recents = add_recent_profile(&[], "a", 1);
        recents = add_recent_profile(&recents, "b", 2);
        recents = add_recent_profile(&recents, "a", 3);
        assert_eq!(
            recents,
            vec![
                RecentProfile {
                    profile_id: "a".to_owned(),
                    last_used_at: 3,
                },
                RecentProfile {
                    profile_id: "b".to_owned(),
                    last_used_at: 2,
                },
            ]
        );
    }

    #[test]
    fn keeps_only_the_newest_ones() {
        let mut recents = add_recent_profile(&[], "first", 0);
        for index in 1..=PROFILE_RECENTS_LIMIT {
            recents = add_recent_profile(&recents, &format!("p{index}"), index as i64);
        }
        assert_eq!(recents.len(), PROFILE_RECENTS_LIMIT);
        assert!(!recents.iter().any(|recent| recent.profile_id == "first"));
    }

    #[test]
    fn forgets_a_profile() {
        let recents = add_recent_profile(&add_recent_profile(&[], "a", 1), "b", 2);
        assert_eq!(
            remove_recent_profile(&recents, "a"),
            vec![RecentProfile {
                profile_id: "b".to_owned(),
                last_used_at: 2,
            }]
        );
    }

    #[test]
    fn loads_only_well_formed_entries() {
        let stored = r#"[{"profileId":"a","lastUsedAt":1},{"profileId":2,"lastUsedAt":1},null]"#;
        assert_eq!(
            parse_recent_profiles(Some(stored)),
            vec![RecentProfile {
                profile_id: "a".to_owned(),
                last_used_at: 1,
            }]
        );
        assert!(parse_recent_profiles(Some("{not json")).is_empty());
        assert!(parse_recent_profiles(None).is_empty());
    }
}
