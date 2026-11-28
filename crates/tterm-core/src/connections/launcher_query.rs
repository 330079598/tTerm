//! Pure ranking for the new-tab launcher.

use std::collections::HashMap;

use crate::connections::profile_recents::RecentProfile;
use crate::profiles::launch::SavedSshProfileSummary;

/// The display-safe saved profile returned to the launcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherProfile {
    pub id: String,
    pub name: String,
    pub endpoint: String,
}

/// Filters and ranks saved SSH profiles without reading storage or opening a connection.
pub fn query_saved_profiles(
    query: &str,
    profiles: &[SavedSshProfileSummary],
    recents: &[RecentProfile],
    limit: usize,
) -> Vec<LauncherProfile> {
    if limit == 0 {
        return Vec::new();
    }
    let recent = recency(recents);
    let normalized = normalize(query);
    let mut ranked = profiles
        .iter()
        .enumerate()
        .filter_map(|(index, profile)| {
            let score = match_score(profile, &normalized)?;
            Some((index, profile, score, recent.get(&profile.id).copied()))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        let left_score = left.2;
        let right_score = right.2;
        right_score
            .cmp(&left_score)
            .then_with(|| right.3.cmp(&left.3))
            .then_with(|| left.0.cmp(&right.0))
    });
    ranked
        .into_iter()
        .take(limit)
        .map(|(_, profile, _, _)| LauncherProfile {
            id: profile.id.clone(),
            name: profile.name.clone(),
            endpoint: endpoint(profile),
        })
        .collect()
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn recency(recents: &[RecentProfile]) -> HashMap<String, i64> {
    recents.iter().fold(HashMap::new(), |mut result, recent| {
        result
            .entry(recent.profile_id.clone())
            .and_modify(|value| *value = (*value).max(recent.last_used_at))
            .or_insert(recent.last_used_at);
        result
    })
}

fn match_score(profile: &SavedSshProfileSummary, query: &str) -> Option<u8> {
    if query.is_empty() {
        return Some(0);
    }
    let fields = [
        profile.name.as_str(),
        profile.group_path.as_deref().unwrap_or_default(),
        profile.host.as_deref().unwrap_or_default(),
        profile.username.as_deref().unwrap_or_default(),
    ];
    let words = query.split_whitespace();
    if !words
        .clone()
        .all(|word| fields.iter().any(|field| normalize(field).contains(word)))
    {
        return None;
    }
    let name = normalize(&profile.name);
    if name == query {
        Some(4)
    } else if name.starts_with(query) {
        Some(3)
    } else {
        Some(2)
    }
}

fn endpoint(profile: &SavedSshProfileSummary) -> String {
    let username = profile
        .username
        .as_deref()
        .filter(|value| !value.is_empty());
    let host = profile.host.as_deref().unwrap_or_default();
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    let mut result = username
        .map(|value| format!("{value}@"))
        .unwrap_or_default();
    result.push_str(&host);
    if let Some(port) = profile.port {
        result.push(':');
        result.push_str(&port.to_string());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str, name: &str) -> SavedSshProfileSummary {
        SavedSshProfileSummary {
            id: id.into(),
            name: name.into(),
            username: Some("root".into()),
            host: Some(format!("{id}.example")),
            port: Some(22),
            group_path: Some("Production".into()),
        }
    }

    fn recent(id: &str, at: i64) -> RecentProfile {
        RecentProfile {
            profile_id: id.into(),
            last_used_at: at,
        }
    }

    #[test]
    fn empty_query_places_existing_recents_first_and_ignores_stale_rows() {
        let profiles = vec![profile("a", "Alpha"), profile("b", "Beta")];
        let result = query_saved_profiles("", &profiles, &[recent("b", 2), recent("gone", 9)], 8);
        assert_eq!(
            result
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "a"]
        );
    }

    #[test]
    fn exact_name_beats_prefix_and_field_match() {
        let profiles = vec![
            profile("field", "Other"),
            profile("prefix", "Web server"),
            profile("exact", "web"),
        ];
        let result = query_saved_profiles("web", &profiles, &[], 8);
        assert_eq!(
            result
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["exact", "prefix"]
        );
    }

    #[test]
    fn group_host_and_username_are_searchable() {
        let mut item = profile("one", "Other");
        item.host = Some("db.internal".into());
        item.username = Some("deploy".into());
        item.group_path = Some("Staging".into());
        assert_eq!(
            query_saved_profiles("staging", &[item.clone()], &[], 8).len(),
            1
        );
        assert_eq!(
            query_saved_profiles("deploy", &[item.clone()], &[], 8).len(),
            1
        );
        assert_eq!(query_saved_profiles("missing", &[item], &[], 8).len(), 0);
    }

    #[test]
    fn ties_keep_saved_order_and_recent_duplicates_use_newest_time() {
        let profiles = vec![profile("a", "Alpha"), profile("b", "Beta")];
        let result = query_saved_profiles("", &profiles, &[recent("a", 1), recent("a", 5)], 8);
        assert_eq!(result[0].id, "a");
    }

    #[test]
    fn limit_is_respected() {
        let profiles = (0..10)
            .map(|index| profile(&index.to_string(), "Host"))
            .collect::<Vec<_>>();
        assert_eq!(query_saved_profiles("", &profiles, &[], 8).len(), 8);
    }

    #[test]
    fn endpoint_contains_user_port_and_brackets_ipv6() {
        let mut item = profile("one", "One");
        item.host = Some("::1".into());
        item.port = Some(2200);
        assert_eq!(
            query_saved_profiles("one", &[item], &[], 8)[0].endpoint,
            "root@[::1]:2200"
        );
    }
}
