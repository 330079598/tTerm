//! Recently used saved connections, from `src/lib/profileRecents.ts`.
//!
//! The WebView keeps its copy in `localStorage`; the native clients share the
//! same camelCase JSON in the `app_meta` row below, so an upgraded Tauri client
//! reads what GPUI recorded.

use crate::db::meta;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

/// When a saved connection last reached a shell or opened its port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProfile {
    /// The saved profile's id.
    pub profile_id: String,
    /// Unix milliseconds of the last use.
    pub last_used_at: i64,
}

/// The `localStorage` key the WebView keeps the list under.
pub const PROFILE_RECENTS_STORAGE_KEY: &str = "tterm.profile-recents.v1";

/// The `app_meta` key shared with the WebView's frontend-state contract.
pub const PROFILE_RECENTS_META_KEY: &str = "frontend.recent_profiles";

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

/// Reads the saved connections used lately.
///
/// This is synchronous and reads the shared database, so callers on the UI
/// thread must move it to a blocking worker.
pub fn read_recent_profiles() -> Result<Vec<RecentProfile>, String> {
    crate::db::read(read_recent_profiles_from)
}

/// Records a successful use of a saved connection.
///
/// The read, update and write share one transaction. A use older than the
/// stored one is ignored, so a delayed writer cannot move the profile back in
/// time or ahead of a newer use.
pub fn record_recent_profile(profile_id: &str, used_at: i64) -> Result<Vec<RecentProfile>, String> {
    let profile_id = profile_id.trim();
    if profile_id.is_empty() {
        return Err("A recent saved connection needs a profile id".to_string());
    }
    crate::db::write(|transaction| record_recent_profile_at(transaction, profile_id, used_at))
}

fn read_recent_profiles_from(connection: &Connection) -> Result<Vec<RecentProfile>, String> {
    let raw = meta::get_raw(connection, PROFILE_RECENTS_META_KEY)?;
    Ok(parse_recent_profiles(raw.as_deref()))
}

fn record_recent_profile_at(
    connection: &Connection,
    profile_id: &str,
    used_at: i64,
) -> Result<Vec<RecentProfile>, String> {
    let mut recents = read_recent_profiles_from(connection)?;
    recents.sort_by_key(|recent| std::cmp::Reverse(recent.last_used_at));
    if recents
        .iter()
        .any(|recent| recent.profile_id == profile_id && recent.last_used_at >= used_at)
    {
        return Ok(recents);
    }

    if recents.len() >= PROFILE_RECENTS_LIMIT
        && recents
            .last()
            .is_some_and(|oldest| oldest.last_used_at > used_at)
    {
        return Ok(recents);
    }

    let mut next = add_recent_profile(&recents, profile_id, used_at);
    next.sort_by_key(|recent| std::cmp::Reverse(recent.last_used_at));
    next.truncate(PROFILE_RECENTS_LIMIT);
    meta::set(connection, PROFILE_RECENTS_META_KEY, &next)?;
    Ok(next)
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
        PROFILE_RECENTS_LIMIT, PROFILE_RECENTS_META_KEY, RecentProfile, add_recent_profile, meta,
        parse_recent_profiles, read_recent_profiles_from, record_recent_profile_at,
        remove_recent_profile,
    };
    use crate::db::Database;

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

    #[test]
    fn stores_the_shared_camel_case_contract_with_one_entry_per_profile() {
        let database = Database::open_in_memory().expect("open in-memory database");
        database
            .write(|connection| record_recent_profile_at(connection, "alpha", 10))
            .unwrap();
        database
            .write(|connection| record_recent_profile_at(connection, "beta", 20))
            .unwrap();
        database
            .write(|connection| record_recent_profile_at(connection, "alpha", 30))
            .unwrap();

        let raw = database
            .read(|connection| meta::get_raw(connection, PROFILE_RECENTS_META_KEY))
            .unwrap()
            .expect("the recents row was written");
        assert_eq!(
            raw,
            r#"[{"profileId":"alpha","lastUsedAt":30},{"profileId":"beta","lastUsedAt":20}]"#
        );
        assert_eq!(
            database.read(read_recent_profiles_from).unwrap(),
            vec![
                RecentProfile {
                    profile_id: "alpha".to_owned(),
                    last_used_at: 30,
                },
                RecentProfile {
                    profile_id: "beta".to_owned(),
                    last_used_at: 20,
                },
            ]
        );
    }

    #[test]
    fn a_delayed_older_use_cannot_regress_the_order() {
        let database = Database::open_in_memory().expect("open in-memory database");
        database
            .write(|connection| record_recent_profile_at(connection, "alpha", 40))
            .unwrap();
        database
            .write(|connection| record_recent_profile_at(connection, "beta", 30))
            .unwrap();
        database
            .write(|connection| record_recent_profile_at(connection, "alpha", 20))
            .unwrap();

        assert_eq!(
            database.read(read_recent_profiles_from).unwrap(),
            vec![
                RecentProfile {
                    profile_id: "alpha".to_owned(),
                    last_used_at: 40,
                },
                RecentProfile {
                    profile_id: "beta".to_owned(),
                    last_used_at: 30,
                },
            ]
        );
    }

    #[test]
    fn keeps_the_cap_and_does_not_let_a_cold_delayed_use_drop_a_newer_one() {
        let database = Database::open_in_memory().expect("open in-memory database");
        for index in 0..PROFILE_RECENTS_LIMIT {
            database
                .write(|connection| {
                    record_recent_profile_at(connection, &format!("p{index}"), index as i64)
                })
                .unwrap();
        }
        database
            .write(|connection| record_recent_profile_at(connection, "cold", -1))
            .unwrap();

        let recents = database.read(read_recent_profiles_from).unwrap();
        assert_eq!(recents.len(), PROFILE_RECENTS_LIMIT);
        assert!(recents.iter().any(|recent| recent.profile_id == "p0"));
        assert!(!recents.iter().any(|recent| recent.profile_id == "cold"));
    }
}

#[cfg(test)]
mod late_tests;
