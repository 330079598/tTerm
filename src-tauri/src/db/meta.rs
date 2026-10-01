//! Small values in `app_meta`: markers and state the app keeps for itself,
//! such as when a backup last ran. Values are stored as JSON text.

use super::sql_error;
use rusqlite::{Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

/// The value under `key`; `None` when it is missing or no longer parses.
pub(crate) fn get<T: DeserializeOwned>(
    connection: &Connection,
    key: &str,
) -> Result<Option<T>, String> {
    let text = connection
        .query_row("SELECT value FROM app_meta WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(sql_error("Failed to read app state"))?;
    Ok(text.and_then(|text| serde_json::from_str(&text).ok()))
}

pub(crate) fn contains(connection: &Connection, key: &str) -> Result<bool, String> {
    connection
        .query_row("SELECT 1 FROM app_meta WHERE key = ?1", [key], |_| Ok(()))
        .optional()
        .map(|row| row.is_some())
        .map_err(sql_error("Failed to read app state"))
}

pub(crate) fn set<T: Serialize + ?Sized>(
    connection: &Connection,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let text = serde_json::to_string(value)
        .map_err(|error| format!("Failed to encode app state '{key}': {error}"))?;
    connection
        .execute(
            "INSERT INTO app_meta (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, &text],
        )
        .map_err(sql_error("Failed to save app state"))?;
    Ok(())
}

pub(crate) fn remove(connection: &Connection, key: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM app_meta WHERE key = ?1", [key])
        .map_err(sql_error("Failed to save app state"))?;
    Ok(())
}

/// Moves `(field, key)` pairs out of the JSON object in `path` into
/// `app_meta`, for state older versions kept in their settings files. A key
/// already in the database wins, and the fields are removed from the file
/// afterwards, so this is a no-op once done.
pub(crate) fn move_file_fields(path: &Path, fields: &[(&str, &str)]) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Failed to read '{}': {error}", path.display()))?;
    let Ok(Value::Object(mut object)) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(());
    };
    let moved: Vec<(&str, Value)> = fields
        .iter()
        .filter_map(|&(field, key)| object.remove(field).map(|value| (key, value)))
        .collect();
    if moved.is_empty() {
        return Ok(());
    }
    super::write(|transaction| {
        for (key, value) in &moved {
            if !value.is_null() && !contains(transaction, key)? {
                set(transaction, key, value)?;
            }
        }
        Ok(())
    })?;
    let content = serde_json::to_vec_pretty(&Value::Object(object))
        .map_err(|error| format!("Failed to encode '{}': {error}", path.display()))?;
    crate::config::atomic_write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn values_round_trip_and_unparsable_ones_read_as_missing() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                assert_eq!(get::<i64>(connection, "a")?, None);
                set(connection, "a", &42_i64)?;
                set(connection, "b", &["x", "y"])?;
                assert_eq!(get::<i64>(connection, "a")?, Some(42));
                assert_eq!(
                    get::<Vec<String>>(connection, "b")?,
                    Some(vec!["x".to_string(), "y".to_string()])
                );
                assert_eq!(get::<i64>(connection, "b")?, None);
                set(connection, "a", &7_i64)?;
                assert_eq!(get::<i64>(connection, "a")?, Some(7));
                remove(connection, "a")?;
                assert!(!contains(connection, "a")?);
                Ok(())
            })
            .unwrap();
    }
}
