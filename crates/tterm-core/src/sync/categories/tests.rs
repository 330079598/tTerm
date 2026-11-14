//! Contract tests from `src/lib/sync.ts` and `ts-oracle.txt`.

use crate::sync::categories::{
    SYNC_APPLIED_EVENT, SYNC_CATEGORIES, SYNC_PULL_INTERVALS_MINS, SYNC_PUSH_INTERVALS_SECS,
    SYNC_SETTINGS_EVENT, SYNC_STATUS_EVENT, SyncCategory, changed_any,
};

#[test]
fn names_every_category_the_backend_reports() {
    assert_eq!(
        SYNC_CATEGORIES
            .into_iter()
            .map(SyncCategory::as_str)
            .collect::<Vec<_>>(),
        [
            "profiles",
            "profileGroups",
            "tunnels",
            "proxies",
            "secrets",
            "commands",
            "knownHosts",
            "settings",
            "themes",
            "serialSends"
        ]
    );
    for category in SYNC_CATEGORIES {
        assert_eq!(SyncCategory::parse(category.as_str()), Some(category));
    }
    assert_eq!(SyncCategory::parse("unknown"), None);
}

#[test]
fn keeps_the_event_names_and_intervals_the_webview_uses() {
    assert_eq!(
        [SYNC_SETTINGS_EVENT, SYNC_APPLIED_EVENT, SYNC_STATUS_EVENT],
        [
            "tterm:sync-settings",
            "tterm:sync-applied",
            "tterm:sync-status"
        ]
    );
    assert_eq!(SYNC_PUSH_INTERVALS_SECS, [30, 60, 300, 900]);
    assert_eq!(SYNC_PULL_INTERVALS_MINS, [5, 15, 30, 60]);
}

#[test]
fn refreshes_only_when_a_watched_category_changed() {
    let changed = [SyncCategory::Profiles, SyncCategory::Themes];
    assert!(changed_any(&changed, &[SyncCategory::Themes]));
    assert!(changed_any(
        &changed,
        &[SyncCategory::Commands, SyncCategory::Profiles]
    ));
    assert!(!changed_any(
        &changed,
        &[SyncCategory::KnownHosts, SyncCategory::Settings]
    ));
    assert!(!changed_any(&[], &[SyncCategory::Profiles]));
}
