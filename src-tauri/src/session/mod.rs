//! The open tabs and layout, restored on the next launch. Kept in the
//! database; older versions kept them in `session.json`.

use crate::core::blocking::run_blocking;
use crate::db::meta;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub(crate) const SESSION_KEY: &str = "session";
/// Set once `session.json` was moved into the database, so the merge of the
/// legacy config folder stops bringing the file back.
const FILE_IMPORTED_KEY: &str = "session_file_imported_at";
const FILE_NAME: &str = "session.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionData {
    pub tabs: serde_json::Value,
    pub active_tab_id: Option<String>,
    #[serde(default)]
    pub layout: Option<serde_json::Value>,
    #[serde(default)]
    pub schema_version: u32,
    pub last_saved: i64,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            tabs: serde_json::json!([]),
            active_tab_id: None,
            layout: None,
            schema_version: 1,
            last_saved: 0,
        }
    }
}

// Re-sanitize session payloads on the backend so no caller can persist raw secrets.
fn sanitize_session_tabs(tabs: serde_json::Value) -> serde_json::Value {
    match tabs {
        serde_json::Value::Array(entries) => serde_json::Value::Array(
            entries
                .into_iter()
                .map(sanitize_session_tab)
                .filter(|tab| !tab.is_null())
                .collect(),
        ),
        _ => serde_json::json!([]),
    }
}

fn sanitize_session_tab(tab: serde_json::Value) -> serde_json::Value {
    let mut tab = match tab {
        serde_json::Value::Object(map) => map,
        _ => return serde_json::Value::Null,
    };

    if let Some(serde_json::Value::Object(connection)) = tab.get_mut("connection") {
        connection.remove("password");
        connection.remove("privateKeyPassphrase");

        if let Some(legacy_jump_host) = connection.remove("jumpHost") {
            let should_adopt_legacy = !matches!(
                connection.get("jumpHosts"),
                Some(serde_json::Value::Array(entries)) if !entries.is_empty()
            );

            if should_adopt_legacy {
                connection.insert(
                    "jumpHosts".to_string(),
                    serde_json::Value::Array(vec![sanitize_jump_host_value(legacy_jump_host)]),
                );
            }
        }

        if let Some(serde_json::Value::Array(jump_hosts)) = connection.get_mut("jumpHosts") {
            for jump_host in jump_hosts.iter_mut() {
                let sanitized = sanitize_jump_host_value(std::mem::take(jump_host));
                *jump_host = sanitized;
            }
        }
    }

    serde_json::Value::Object(tab)
}

fn sanitize_jump_host_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut jump_host) => {
            jump_host.remove("password");
            jump_host.remove("privateKeyPassphrase");
            serde_json::Value::Object(jump_host)
        }
        other => other,
    }
}

/// The saved session; `None` when there is none.
pub(crate) fn read_session(connection: &Connection) -> Result<Option<SessionData>, String> {
    Ok(
        meta::get::<SessionData>(connection, SESSION_KEY)?.map(|mut session| {
            // Clean older sessions as they are loaded so stale plaintext secrets are dropped.
            session.tabs = sanitize_session_tabs(session.tabs);
            session
        }),
    )
}

pub(crate) fn write_session(
    connection: &Connection,
    mut session: SessionData,
) -> Result<(), String> {
    // Keep a backend-side guard even if the frontend payload changes in the future.
    session.tabs = sanitize_session_tabs(session.tabs);
    meta::set(connection, SESSION_KEY, &session)
}

fn clear(connection: &Connection) -> Result<(), String> {
    meta::remove(connection, SESSION_KEY)
}

#[tauri::command]
pub async fn load_session() -> Result<SessionData, String> {
    run_blocking(|| crate::db::read(read_session).map(Option::unwrap_or_default)).await
}

#[tauri::command]
pub async fn save_session(session: SessionData) -> Result<(), String> {
    run_blocking(move || crate::db::write(|transaction| write_session(transaction, session))).await
}

#[tauri::command]
pub async fn clear_session() -> Result<(), String> {
    run_blocking(|| crate::db::write(|transaction| clear(transaction))).await
}

/// Moves `session.json` from older versions into the database.
pub(crate) fn import_file(
    database: &crate::db::Database,
    config_dir: &std::path::Path,
) -> Result<(), String> {
    meta::import_file(database, &config_dir.join(FILE_NAME), import_session)?;
    database.write(|transaction| {
        meta::set_if_absent(
            transaction,
            FILE_IMPORTED_KEY,
            &chrono::Utc::now().timestamp_millis(),
        )
    })
}

/// Whether `session.json` was moved into the database.
pub(crate) fn file_imported() -> bool {
    crate::db::read(|connection| meta::contains(connection, FILE_IMPORTED_KEY)).unwrap_or(false)
}

fn import_session(connection: &Connection, bytes: &[u8]) -> Result<(), String> {
    match meta::parse_file::<SessionData>(bytes) {
        Ok(session) if !meta::contains(connection, SESSION_KEY)? => {
            write_session(connection, session)
        }
        Ok(_) => Ok(()),
        // A session is not worth keeping a broken file around for.
        Err(error) => {
            eprintln!("Dropping unreadable session.json: {error}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn sessions_round_trip_without_secrets() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                assert!(read_session(connection)?.is_none());
                let session: SessionData = serde_json::from_value(serde_json::json!({
                    "tabs": [{"id": "1", "connection": {"host": "h", "password": "secret"}}],
                    "active_tab_id": "1",
                    "last_saved": 5
                }))
                .unwrap();
                write_session(connection, session)?;
                let saved = read_session(connection)?.unwrap();
                assert_eq!(saved.active_tab_id.as_deref(), Some("1"));
                assert_eq!(saved.last_saved, 5);
                assert_eq!(saved.tabs[0]["connection"]["host"], "h");
                assert!(saved.tabs[0]["connection"].get("password").is_none());
                clear(connection)?;
                assert!(read_session(connection)?.is_none());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn importing_a_session_file_keeps_a_newer_one_and_drops_a_broken_one() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                let file = br#"{"tabs":[{"id":"old"}],"active_tab_id":"old","last_saved":1}"#;
                import_session(connection, file)?;
                assert_eq!(
                    read_session(connection)?.unwrap().active_tab_id.as_deref(),
                    Some("old")
                );
                let newer = SessionData {
                    active_tab_id: Some("new".to_string()),
                    ..SessionData::default()
                };
                write_session(connection, newer)?;
                import_session(connection, file)?;
                assert_eq!(
                    read_session(connection)?.unwrap().active_tab_id.as_deref(),
                    Some("new")
                );
                import_session(connection, b"{ not json")?;
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn legacy_session_without_layout_uses_defaults() {
        let session: SessionData =
            serde_json::from_str(r#"{"tabs":[],"active_tab_id":null,"last_saved":0}"#)
                .expect("legacy session should deserialize");

        assert!(session.layout.is_none());
        assert_eq!(session.schema_version, 0);
    }

    #[test]
    fn sanitize_session_tabs_migrates_legacy_jump_host_to_jump_hosts() {
        let tabs = serde_json::json!([
            {
                "id": "1",
                "connection": {
                    "jumpHost": {
                        "host": "bastion",
                        "port": 22,
                        "username": "stone",
                        "password": "secret"
                    }
                }
            }
        ]);

        let sanitized = sanitize_session_tabs(tabs);
        let connection = sanitized[0]["connection"]
            .as_object()
            .expect("connection object");
        assert!(connection.get("jumpHost").is_none());
        let jump_hosts = connection
            .get("jumpHosts")
            .and_then(|value| value.as_array())
            .expect("jumpHosts array");
        assert_eq!(jump_hosts.len(), 1);
        assert_eq!(jump_hosts[0]["host"], "bastion");
        assert!(jump_hosts[0].get("password").is_none());
    }
}
