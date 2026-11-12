//! The tabs put away from the tab bar to be opened again later, kept across
//! launches. The frontend owns their shape; the backend only stores them and
//! drops any secret a tab carries, as it does for the session.
//!
//! What each terminal showed when it was put away is kept as a gzip file in
//! `collapsed-scrollback/`, apart from the session's `scrollback/`, whose
//! files go with the tabs of the session and with its setting.

use crate::core::blocking::run_blocking;
use crate::db::meta;
use crate::session::scrollback::{self, SavedScrollback, ScrollbackSnapshot};
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;

const COLLAPSED_TABS_KEY: &str = "collapsed_tabs";
const CONTENT_DIR_NAME: &str = "collapsed-scrollback";

fn content_dir() -> Result<PathBuf, String> {
    Ok(crate::config::paths::get_config_path()?.join(CONTENT_DIR_NAME))
}

/// The keys of the terminal content the sets still refer to.
fn content_keys(sets: &Value) -> HashSet<String> {
    sets.as_array()
        .into_iter()
        .flatten()
        .filter_map(|set| set.get("groups")?.as_array())
        .flatten()
        .filter_map(|group| group.get("contentKeys")?.as_array())
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

/// Drops the secrets of every tab of every set, wherever the sets keep them.
fn sanitize_sets(sets: Value) -> Value {
    let Value::Array(sets) = sets else {
        return Value::Array(Vec::new());
    };
    Value::Array(
        sets.into_iter()
            .map(|mut set| {
                if let Some(Value::Array(groups)) = set.get_mut("groups") {
                    for group in groups.iter_mut() {
                        if let Some(tabs) = group.get_mut("tabs") {
                            *tabs = super::sanitize_session_tabs(std::mem::take(tabs));
                        }
                    }
                }
                set
            })
            .collect(),
    )
}

fn read(connection: &Connection) -> Result<Value, String> {
    Ok(meta::get::<Value>(connection, COLLAPSED_TABS_KEY)?
        .map(sanitize_sets)
        .unwrap_or_else(|| Value::Array(Vec::new())))
}

fn write(connection: &Connection, sets: Value) -> Result<(), String> {
    let sets = sanitize_sets(sets);
    match &sets {
        Value::Array(entries) if entries.is_empty() => meta::remove(connection, COLLAPSED_TABS_KEY),
        _ => meta::set(connection, COLLAPSED_TABS_KEY, &sets),
    }
}

/// The tabs put away, the newest set first; an empty list when there are none.
#[tauri::command]
pub async fn load_collapsed_tabs() -> Result<Value, String> {
    run_blocking(|| crate::db::read(read)).await
}

/// Replaces the tabs put away with `sets`, and deletes the terminal content
/// of the tabs no longer among them.
#[tauri::command]
pub async fn save_collapsed_tabs(sets: Value) -> Result<(), String> {
    run_blocking(move || {
        let keep = content_keys(&sets);
        crate::db::write(|transaction| write(transaction, sets))?;
        scrollback::prune(&content_dir()?, &keep)
    })
    .await
}

/// Keeps what the terminals of tabs being put away showed, each under its key.
#[tauri::command]
pub async fn save_collapsed_content(snapshots: Vec<ScrollbackSnapshot>) -> Result<(), String> {
    run_blocking(move || scrollback::write_snapshots(&content_dir()?, &snapshots)).await
}

/// The content kept under `keys`, for tabs opened again; it is deleted as it is read.
#[tauri::command]
pub async fn take_collapsed_content(keys: Vec<String>) -> Result<Vec<SavedScrollback>, String> {
    run_blocking(move || scrollback::take_snapshots(&content_dir()?, &keys)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use serde_json::json;

    #[test]
    fn sets_round_trip_without_secrets() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                assert_eq!(read(connection)?, json!([]));
                let sets = json!([{
                    "id": "s",
                    "collapsedAt": 3,
                    "groups": [{
                        "id": "g",
                        "shape": { "type": "pane" },
                        "activeIndex": 0,
                        "tabs": [{
                            "type": "ssh",
                            "connection": {
                                "host": "h",
                                "password": "secret",
                                "jumpHosts": [{ "host": "j", "privateKeyPassphrase": "p" }]
                            }
                        }]
                    }]
                }]);
                write(connection, sets)?;
                let saved = read(connection)?;
                let tab = &saved[0]["groups"][0]["tabs"][0];
                assert_eq!(tab["connection"]["host"], "h");
                assert!(tab["connection"].get("password").is_none());
                assert!(tab["connection"]["jumpHosts"][0]
                    .get("privateKeyPassphrase")
                    .is_none());
                assert_eq!(saved[0]["collapsedAt"], 3);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn content_keys_are_read_from_every_group() {
        let sets = json!([
            { "groups": [{ "contentKeys": ["a", null] }, { "contentKeys": ["b"] }] },
            { "groups": [{ "tabs": [] }] },
        ]);
        let keys = content_keys(&sets);
        assert_eq!(keys.len(), 2);
        assert!(keys.contains("a") && keys.contains("b"));
    }

    #[test]
    fn saving_no_sets_clears_them() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                write(connection, json!([{ "id": "s", "groups": [] }]))?;
                write(connection, json!([]))?;
                assert!(!meta::contains(connection, COLLAPSED_TABS_KEY)?);
                write(connection, json!("not a list"))?;
                assert_eq!(read(connection)?, json!([]));
                Ok(())
            })
            .unwrap();
    }
}
