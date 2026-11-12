use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::RemoteCwdStore;

#[test]
fn keeps_the_last_directory_per_tab_until_its_session_is_gone() {
    let mut store = RemoteCwdStore::new();
    store.set_tab_remote_cwd("tab-a", Some("/srv"));
    store.set_tab_remote_cwd("tab-b", Some("/tmp"));
    store.set_tab_remote_cwd("tab-a", Some("/var/log"));
    assert_eq!(store.get_tab_remote_cwd("tab-a"), Some("/var/log"));
    assert_eq!(store.get_tab_remote_cwd("tab-b"), Some("/tmp"));

    store.set_tab_remote_cwd("tab-a", None);
    assert_eq!(store.get_tab_remote_cwd("tab-a"), None);
    store.set_tab_remote_cwd("tab-b", None);
}

#[test]
fn re_renders_the_tabs_subscribers_when_its_directory_changes() {
    let mut store = RemoteCwdStore::new();
    let notifications = Arc::new(AtomicUsize::new(0));
    let counter = notifications.clone();
    store.subscribe(move || {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(store.get_tab_remote_cwd("tab-c"), None);

    assert!(store.set_tab_remote_cwd("tab-c", Some("/etc")));
    assert_eq!(store.get_tab_remote_cwd("tab-c"), Some("/etc"));
    assert_eq!(notifications.load(Ordering::Relaxed), 1);

    assert!(store.set_tab_remote_cwd("tab-c", None));
    assert_eq!(store.get_tab_remote_cwd("tab-c"), None);
    assert_eq!(notifications.load(Ordering::Relaxed), 2);
}

#[test]
fn keeps_the_other_host_a_tabs_terminal_is_on_apart_from_its_directory() {
    let mut store = RemoteCwdStore::new();
    store.set_tab_remote_cwd("tab-d", Some("/srv"));
    let notifications = Arc::new(AtomicUsize::new(0));
    let counter = notifications.clone();
    store.subscribe(move || {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(store.get_tab_remote_away_host("tab-d"), None);

    assert!(store.set_tab_remote_away_host("tab-d", Some("db01")));
    assert_eq!(store.get_tab_remote_away_host("tab-d"), Some("db01"));
    assert_eq!(store.get_tab_remote_cwd("tab-d"), Some("/srv"));
    assert_eq!(notifications.load(Ordering::Relaxed), 1);

    assert!(store.set_tab_remote_away_host("tab-d", None));
    assert_eq!(store.get_tab_remote_away_host("tab-d"), None);
    assert_eq!(notifications.load(Ordering::Relaxed), 2);
    store.set_tab_remote_cwd("tab-d", None);
}

#[test]
fn keeps_the_connected_hostname_without_notifying() {
    let mut store = RemoteCwdStore::new();
    let notifications = Arc::new(AtomicUsize::new(0));
    let counter = notifications.clone();
    let listener = store.subscribe(move || {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    store.set_tab_remote_hostname("tab-e", Some("web01"));
    assert_eq!(store.get_tab_remote_hostname("tab-e"), Some("web01"));
    assert_eq!(notifications.load(Ordering::Relaxed), 0);
    store.set_tab_remote_hostname("tab-e", None);
    assert_eq!(store.get_tab_remote_hostname("tab-e"), None);
    store.unsubscribe(listener);
    store.set_tab_remote_cwd("tab-e", Some("/srv"));
    assert_eq!(notifications.load(Ordering::Relaxed), 0);
}

#[test]
fn setting_the_same_value_does_not_notify() {
    let mut store = RemoteCwdStore::new();
    let notifications = Arc::new(AtomicUsize::new(0));
    let counter = notifications.clone();
    store.subscribe(move || {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    assert!(store.set_tab_remote_cwd("tab-f", Some("/srv")));
    assert!(!store.set_tab_remote_cwd("tab-f", Some("/srv")));
    assert_eq!(notifications.load(Ordering::Relaxed), 1);
    assert!(!store.set_tab_remote_away_host("tab-f", None));
    assert_eq!(notifications.load(Ordering::Relaxed), 1);
}
