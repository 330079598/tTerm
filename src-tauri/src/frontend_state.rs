//! User data the web view used to keep in localStorage.
//!
//! Each value is one `app_meta` row under a `frontend.` key. Only the keys in
//! this module are accepted, so the commands are not a general key-value
//! store. The first start after the upgrade copies the localStorage values
//! into the database. Later starts compare a per-key fingerprint with the
//! old copy, importing it again if an older version changed it, and every
//! new value is written back to localStorage so a rollback sees it too.
//!
//! The GPUI build shares these rows and must follow the same contract. It
//! must not delete a `frontend.` row: clearing a value means writing the
//! empty value with the same shape (an empty array, `null` where the shape
//! allows it, or `false`). It must not write `frontend.migrated` or any
//! `frontend.synced.*` fingerprint; a marker without the matching
//! fingerprints makes a later Tauri start import stale localStorage values.
//! If the marker exists and one of these rows is missing or cannot be parsed,
//! the next Tauri start reimports that key from localStorage.
//!
//! Removing the localStorage originals is planned for 0.10.0, one release
//! after the 0.9.0 transition build, once users have had time to upgrade.

use crate::core::blocking::run_blocking;
use crate::db::meta;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

/// Set once the localStorage values were handed over or found unnecessary.
const MIGRATED_KEY: &str = "frontend.migrated";
const MIGRATION_VERSION: u32 = 1;

/// The command key, its `app_meta` key and the row last synced to localStorage.
const STATE_KEYS: [(&str, &str, &str); 9] = [
    (
        "recentCommands",
        "frontend.recent_commands",
        "frontend.synced.recent_commands",
    ),
    (
        "recentQuickConnections",
        "frontend.recent_quick_connections",
        "frontend.synced.recent_quick_connections",
    ),
    (
        "recentProfiles",
        "frontend.recent_profiles",
        "frontend.synced.recent_profiles",
    ),
    (
        "sftpColumnWidths",
        "frontend.sftp_column_widths",
        "frontend.synced.sftp_column_widths",
    ),
    (
        "sftpView",
        "frontend.sftp_view",
        "frontend.synced.sftp_view",
    ),
    (
        "serialSendEcho",
        "frontend.serial_send_echo",
        "frontend.synced.serial_send_echo",
    ),
    (
        "commandComposerOpen",
        "frontend.command_composer_open",
        "frontend.synced.command_composer_open",
    ),
    (
        "commandComposerLineByLine",
        "frontend.command_composer_line_by_line",
        "frontend.synced.command_composer_line_by_line",
    ),
    (
        "replayView",
        "frontend.replay_view",
        "frontend.synced.replay_view",
    ),
];

/// The keys an old or new backup's `frontendState` carries. The backup format
/// keeps only these five; the other user data is stored but not backed up.
pub(crate) const BACKUP_KEYS: [(&str, &str); 5] = [
    ("recentCommands", "frontend.recent_commands"),
    (
        "recentQuickConnections",
        "frontend.recent_quick_connections",
    ),
    ("recentProfiles", "frontend.recent_profiles"),
    ("sftpColumnWidths", "frontend.sftp_column_widths"),
    ("sftpView", "frontend.sftp_view"),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationMarker {
    version: u32,
    migrated_at: i64,
}

fn state_key(key: &str) -> Option<&'static str> {
    STATE_KEYS
        .iter()
        .find(|(name, _, _)| *name == key)
        .map(|(_, storage_key, _)| *storage_key)
}

fn fingerprint_key(key: &str) -> Option<&'static str> {
    STATE_KEYS
        .iter()
        .find(|(name, _, _)| *name == key)
        .map(|(_, _, fingerprint_key)| *fingerprint_key)
}

fn canonical(value: &Value) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("Failed to encode frontend state for comparison: {error}"))
}

fn has_valid_shape(key: &str, value: &Value) -> bool {
    match key {
        "recentCommands" | "recentQuickConnections" | "recentProfiles" => value.is_array(),
        "sftpColumnWidths" => value.is_array() || value.is_null(),
        "serialSendEcho" => value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string)),
        "sftpView" | "replayView" => value.is_object() || value.is_null(),
        "commandComposerOpen" | "commandComposerLineByLine" => value.is_boolean(),
        _ => false,
    }
}

/// Every stored value that still has the expected shape, by command key.
pub(crate) fn read_all(connection: &Connection) -> Result<BTreeMap<String, Value>, String> {
    let mut state = BTreeMap::new();
    for (key, storage_key, _) in STATE_KEYS {
        if let Some(value) = stored_value(connection, key, storage_key)? {
            state.insert(key.to_string(), value);
        }
    }
    Ok(state)
}

fn stored_value(
    connection: &Connection,
    key: &str,
    storage_key: &str,
) -> Result<Option<Value>, String> {
    let Some(value) = meta::get::<Value>(connection, storage_key)? else {
        return Ok(None);
    };
    Ok(has_valid_shape(key, &value).then_some(value))
}

fn store_fingerprint(connection: &Connection, key: &str, value: &Value) -> Result<(), String> {
    let fingerprint_key =
        fingerprint_key(key).ok_or_else(|| format!("Unknown frontend state key '{key}'."))?;
    meta::set(connection, fingerprint_key, &canonical(value)?)
}

/// The five fields a backup carries, with the defaults older backups had.
pub(crate) fn backup_state(connection: &Connection) -> Result<Value, String> {
    let mut state = read_all(connection)?;
    Ok(json!({
        "recentCommands": state.remove("recentCommands").unwrap_or_else(|| json!([])),
        "recentQuickConnections": state.remove("recentQuickConnections").unwrap_or_else(|| json!([])),
        "recentProfiles": state.remove("recentProfiles").unwrap_or_else(|| json!([])),
        "sftpColumnWidths": state.remove("sftpColumnWidths").unwrap_or(Value::Null),
        "sftpView": state.remove("sftpView").unwrap_or(Value::Null),
    }))
}

/// Writes the fields a backup carries into the database. Fields that are
/// absent stay as this device has them, as the old localStorage restore did.
pub(crate) fn apply_backup_state(connection: &Connection, state: &Value) -> Result<(), String> {
    let Some(fields) = state.as_object() else {
        return Ok(());
    };
    for (key, storage_key) in BACKUP_KEYS {
        let Some(value) = fields.get(key) else {
            continue;
        };
        // The old localStorage restore left the device's value alone when a
        // backup had no value for a field.
        if !value.is_null() && has_valid_shape(key, value) {
            meta::set(connection, storage_key, value)?;
        }
    }
    Ok(())
}

/// Imports the values localStorage has on the first start, once. Existing
/// backend rows win on that first start: without a marker they can only have
/// come from GPUI or from an import in a degraded session, so they are newer
/// than the 0.8.x copy. Only missing keys are filled from localStorage.
fn migrate(
    connection: &Connection,
    legacy: &HashMap<String, String>,
) -> Result<BTreeMap<String, Value>, String> {
    let mut requested = HashMap::with_capacity(legacy.len());
    for (key, raw) in legacy {
        if state_key(key).is_none() {
            eprintln!("Ignoring unknown frontend state key '{key}' from localStorage.");
            continue;
        }
        requested.insert(key.as_str(), raw.as_str());
    }

    let migrated = meta::contains(connection, MIGRATED_KEY)?;
    for (key, storage_key, _) in STATE_KEYS {
        let Some(raw) = requested.get(key) else {
            if !migrated {
                if let Some(stored) = stored_value(connection, key, storage_key)? {
                    store_fingerprint(connection, key, &stored)?;
                }
            }
            continue;
        };
        let parsed = serde_json::from_str::<Value>(raw)
            .ok()
            .filter(|value| has_valid_shape(key, value));
        let stored = stored_value(connection, key, storage_key)?;
        let Some(value) = parsed else {
            if !migrated {
                if let Some(stored) = stored {
                    store_fingerprint(connection, key, &stored)?;
                }
            }
            continue;
        };
        if !migrated {
            if let Some(stored) = stored.as_ref() {
                store_fingerprint(connection, key, stored)?;
            } else {
                meta::set(connection, storage_key, &value)?;
                store_fingerprint(connection, key, &value)?;
            }
            continue;
        }

        let fingerprint_key =
            fingerprint_key(key).ok_or_else(|| format!("Unknown frontend state key '{key}'."))?;
        let canonical_value = canonical(&value)?;
        let fingerprint = meta::get::<String>(connection, fingerprint_key)?;
        if stored.is_some() && fingerprint.as_deref() == Some(&canonical_value) {
            continue;
        }
        if stored.as_ref() == Some(&value) {
            store_fingerprint(connection, key, &value)?;
            continue;
        }
        if stored.is_none() {
            eprintln!(
                "Re-importing frontend state '{key}' because the stored value is missing or invalid."
            );
        } else if fingerprint.as_deref() != Some(&canonical_value) {
            eprintln!(
                "Re-importing frontend state '{key}' from localStorage after an older version changed it."
            );
        }
        if stored.as_ref() != Some(&value) {
            meta::set(connection, storage_key, &value)?;
        }
        store_fingerprint(connection, key, &value)?;
    }

    if !migrated {
        meta::set(
            connection,
            MIGRATED_KEY,
            &MigrationMarker {
                version: MIGRATION_VERSION,
                migrated_at: chrono::Utc::now().timestamp_millis(),
            },
        )?;
    }
    read_all(connection)
}

fn write_synced_value(connection: &Connection, key: &str, value: &Value) -> Result<(), String> {
    let storage_key =
        state_key(key).ok_or_else(|| format!("Unknown frontend state key '{key}'."))?;
    if !has_valid_shape(key, value) {
        return Err(format!(
            "Frontend state '{key}' has a value with the wrong type."
        ));
    }
    meta::set(connection, storage_key, value)
}

fn acknowledge_synced_value(
    connection: &Connection,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    let storage_key =
        state_key(key).ok_or_else(|| format!("Unknown frontend state key '{key}'."))?;
    if stored_value(connection, key, storage_key)?.as_ref() != Some(value) {
        return Err(format!(
            "Frontend state '{key}' changed before its localStorage copy was written."
        ));
    }
    store_fingerprint(connection, key, value)
}

pub(crate) fn backup_state_for_app() -> Result<Value, String> {
    crate::db::read(backup_state)
}

/// Writes one value. A key outside the whitelist is rejected.
#[tauri::command]
pub async fn write_frontend_state(key: String, value: Value) -> Result<(), String> {
    run_blocking(move || {
        crate::db::write(|transaction| write_synced_value(transaction, &key, &value))
    })
    .await
}

/// Records that the value is now also in localStorage. The frontend calls
/// this only after the old-format copy was written, so a failed copy leaves
/// the previous fingerprint in place and the backend stays authoritative.
#[tauri::command]
pub async fn acknowledge_frontend_state_sync(key: String, value: Value) -> Result<(), String> {
    run_blocking(move || {
        crate::db::write(|transaction| acknowledge_synced_value(transaction, &key, &value))
    })
    .await
}

/// Reads every stored value at startup.
#[tauri::command]
pub async fn load_frontend_state() -> Result<BTreeMap<String, Value>, String> {
    run_blocking(|| crate::db::read(read_all)).await
}

/// Hands the raw localStorage values over on the first start, then returns
/// everything stored. Once the marker exists the legacy values are ignored.
#[tauri::command]
pub async fn migrate_frontend_state(
    legacy: HashMap<String, String>,
) -> Result<BTreeMap<String, Value>, String> {
    run_blocking(move || crate::db::write(|transaction| migrate(transaction, &legacy))).await
}

/// Collects the `app_meta` keys an import of the backup's frontend state may
/// change, for the rollback snapshot.
pub(crate) fn backup_meta_keys() -> impl Iterator<Item = &'static str> {
    BACKUP_KEYS.iter().map(|(_, storage_key)| *storage_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn legacy_state() -> HashMap<String, String> {
        HashMap::from([
            (
                "recentCommands".to_string(),
                json!([{"id": "c1", "commandText": "ls"}]).to_string(),
            ),
            (
                "recentProfiles".to_string(),
                json!([{"profileId": "p1", "lastUsedAt": 1}]).to_string(),
            ),
            (
                "sftpView".to_string(),
                json!({"showHidden": false}).to_string(),
            ),
            ("commandComposerOpen".to_string(), "true".to_string()),
        ])
    }

    fn stored_fingerprint(connection: &Connection, key: &str) -> Option<String> {
        meta::get(
            connection,
            fingerprint_key(key).expect("the test key is in the whitelist"),
        )
        .unwrap()
    }

    #[test]
    fn first_start_moves_valid_values_and_marks_the_migration() {
        let database = Database::open_in_memory().unwrap();
        let state = database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();
        assert_eq!(state["recentCommands"][0]["id"], "c1");
        assert_eq!(state["recentProfiles"][0]["profileId"], "p1");
        assert_eq!(state["sftpView"]["showHidden"], false);
        assert_eq!(state["commandComposerOpen"], true);
        let marker = database
            .read(|connection| meta::get::<Value>(connection, MIGRATED_KEY))
            .unwrap()
            .unwrap();
        assert_eq!(marker["version"], MIGRATION_VERSION);
        assert!(marker["migratedAt"].as_i64().unwrap_or_default() > 0);
        database
            .read(|connection| {
                assert_eq!(
                    stored_fingerprint(connection, "recentCommands"),
                    Some(canonical(&state["recentCommands"]).unwrap())
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn a_stored_value_that_no_longer_parses_is_replaced_by_the_legacy_value() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                meta::put_raw(connection, "frontend.recent_commands", Some("{broken"))
            })
            .unwrap();
        let state = database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();
        assert_eq!(state["recentCommands"][0]["id"], "c1");
    }

    #[test]
    fn existing_backend_values_win_on_the_first_migration() {
        let database = Database::open_in_memory().unwrap();
        let backend = json!([{"id": "from-gpui"}]);
        database
            .write(|connection| meta::set(connection, "frontend.recent_commands", &backend))
            .unwrap();

        let state = database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();

        assert_eq!(state["recentCommands"], backend);
        assert_eq!(state["recentProfiles"][0]["profileId"], "p1");
        database
            .read(|connection| {
                assert_eq!(
                    stored_fingerprint(connection, "recentCommands"),
                    Some(canonical(&backend).unwrap())
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn an_unreadable_stored_value_is_reimported_after_the_migration_marker_exists() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();
        database
            .write(|connection| {
                meta::put_raw(connection, "frontend.recent_commands", Some("{broken"))
            })
            .unwrap();
        let state = database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();
        assert_eq!(state["recentCommands"][0]["id"], "c1");
    }

    #[test]
    fn an_older_versions_change_is_imported_again() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();
        let mut changed = legacy_state();
        changed.insert(
            "recentCommands".to_string(),
            json!([{"id": "newer"}]).to_string(),
        );
        let state = database
            .write(|connection| migrate(connection, &changed))
            .unwrap();
        assert_eq!(state["recentCommands"][0]["id"], "newer");
        database
            .read(|connection| {
                assert_eq!(
                    stored_fingerprint(connection, "recentCommands"),
                    Some(canonical(&json!([{"id": "newer"}])).unwrap())
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn a_local_copy_left_by_a_failed_mirror_does_not_override_the_backend() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();
        let newer = json!([{"id": "newer"}]);
        database
            .write(|connection| write_synced_value(connection, "recentCommands", &newer))
            .unwrap();

        let state = database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();
        assert_eq!(state["recentCommands"][0]["id"], "newer");
    }

    #[test]
    fn a_matching_fingerprint_does_not_reimport_the_legacy_value() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();
        let newer = json!([{"id": "newer"}]);
        database
            .write(|connection| {
                write_synced_value(connection, "recentCommands", &newer)?;
                acknowledge_synced_value(connection, "recentCommands", &newer)
            })
            .unwrap();
        let mut local = legacy_state();
        local.insert("recentCommands".to_string(), newer.to_string());

        let state = database
            .write(|connection| migrate(connection, &local))
            .unwrap();
        assert_eq!(state["recentCommands"][0]["id"], "newer");
    }

    #[test]
    fn a_corrupt_legacy_value_does_not_override_the_backend() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();
        let newer = json!([{"id": "newer"}]);
        database
            .write(|connection| {
                write_synced_value(connection, "recentCommands", &newer)?;
                acknowledge_synced_value(connection, "recentCommands", &newer)
            })
            .unwrap();
        let mut local = legacy_state();
        local.remove("recentCommands");

        let state = database
            .write(|connection| migrate(connection, &local))
            .unwrap();
        assert_eq!(state["recentCommands"][0]["id"], "newer");
    }

    #[test]
    fn acknowledging_a_stale_value_does_not_replace_the_fingerprint() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| migrate(connection, &legacy_state()))
            .unwrap();
        let before = database
            .read(|connection| Ok(stored_fingerprint(connection, "recentCommands")))
            .unwrap();
        let error = database
            .write(|connection| acknowledge_synced_value(connection, "recentCommands", &json!([])))
            .unwrap_err();
        assert!(error.contains("changed"), "{error}");
        assert_eq!(
            database
                .read(|connection| Ok(stored_fingerprint(connection, "recentCommands")))
                .unwrap(),
            before
        );
    }

    #[test]
    fn an_unknown_legacy_key_is_skipped_without_stopping_the_migration() {
        let database = Database::open_in_memory().unwrap();
        let mut state = legacy_state();
        state.insert("anything".to_string(), "1".to_string());
        let migrated = database
            .write(|connection| migrate(connection, &state))
            .unwrap();
        assert_eq!(migrated["recentCommands"][0]["id"], "c1");
        assert!(!migrated.contains_key("anything"));
        let marker = database
            .read(|connection| meta::get::<Value>(connection, MIGRATED_KEY))
            .unwrap();
        assert!(marker.is_some());
    }

    #[test]
    fn invalid_json_and_wrong_types_are_skipped_without_stopping_the_others() {
        let database = Database::open_in_memory().unwrap();
        let state = database
            .write(|connection| {
                migrate(
                    connection,
                    &HashMap::from([
                        ("recentCommands".to_string(), "{not json".to_string()),
                        (
                            "recentProfiles".to_string(),
                            json!("not an array").to_string(),
                        ),
                        (
                            "recentQuickConnections".to_string(),
                            json!([{"host": "example.com"}]).to_string(),
                        ),
                        ("commandComposerOpen".to_string(), "1".to_string()),
                    ]),
                )
            })
            .unwrap();
        assert!(!state.contains_key("recentCommands"));
        assert!(!state.contains_key("recentProfiles"));
        assert_eq!(state["recentQuickConnections"][0]["host"], "example.com");
        assert!(!state.contains_key("commandComposerOpen"));
        let marker = database
            .read(|connection| meta::get::<Value>(connection, MIGRATED_KEY))
            .unwrap();
        assert!(marker.is_some());
    }

    #[test]
    fn one_key_reads_and_writes_and_an_unknown_key_is_rejected() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| meta::set(connection, "frontend.sftp_view", &json!(null)))
            .unwrap();
        let value = database
            .read(|connection| meta::get::<Value>(connection, "frontend.sftp_view"))
            .unwrap();
        assert_eq!(value, Some(Value::Null));
        assert!(has_valid_shape("sftpView", &json!(null)));
        assert!(state_key("anything").is_none());
        assert!(state_key("recentCommands").is_some());
        let error = database
            .write(|connection| write_synced_value(connection, "anything", &json!(1)))
            .unwrap_err();
        assert!(error.contains("anything"), "{error}");
    }

    #[test]
    fn the_backup_state_has_the_same_fields_and_defaults_as_before() {
        let database = Database::open_in_memory().unwrap();
        let empty = database.read(backup_state).unwrap();
        assert_eq!(
            empty,
            json!({
                "recentCommands": [],
                "recentQuickConnections": [],
                "recentProfiles": [],
                "sftpColumnWidths": null,
                "sftpView": null,
            })
        );

        database
            .write(|connection| {
                meta::set(
                    connection,
                    "frontend.recent_commands",
                    &json!([{"id": "c"}]),
                )?;
                meta::set(connection, "frontend.sftp_column_widths", &json!([1, 2]))?;
                meta::set(connection, "frontend.serial_send_echo", &json!(["port:1"]))
            })
            .unwrap();
        let state = database.read(backup_state).unwrap();
        assert_eq!(state["recentCommands"], json!([{"id": "c"}]));
        assert_eq!(state["sftpColumnWidths"], json!([1, 2]));
        assert!(state.get("serialSendEcho").is_none());
    }

    #[test]
    fn importing_a_backup_writes_only_the_fields_it_carries() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                meta::set(
                    connection,
                    "frontend.recent_profiles",
                    &json!([{"profileId": "old"}]),
                )
            })
            .unwrap();
        database
            .write(|connection| {
                apply_backup_state(
                    connection,
                    &json!({
                        "recentCommands": [{"id": "imported"}],
                        "sftpView": null,
                        "customThemes": [{"id": "ignored"}],
                    }),
                )
            })
            .unwrap();
        let state = database.read(read_all).unwrap();
        assert_eq!(state["recentCommands"][0]["id"], "imported");
        assert_eq!(state["recentProfiles"][0]["profileId"], "old");
        assert!(!state.contains_key("sftpView"));
        assert!(!state.contains_key("customThemes"));
        assert_eq!(backup_meta_keys().count(), 5);
    }

    #[test]
    fn values_that_no_longer_parse_read_as_missing() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| meta::put_raw(connection, "frontend.sftp_view", Some("{broken")))
            .unwrap();
        assert!(!database.read(read_all).unwrap().contains_key("sftpView"));
    }
}
