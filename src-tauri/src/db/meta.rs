//! Small values in `app_meta`: markers, state the app keeps for itself
//! (such as when a backup last ran), the session and the backup and sync
//! settings. Values are stored as JSON text.

use super::{sql_error, Database};
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

/// The stored text under `key`, for restoring it later with [`put_raw`].
pub(crate) fn get_raw(connection: &Connection, key: &str) -> Result<Option<String>, String> {
    connection
        .query_row("SELECT value FROM app_meta WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(sql_error("Failed to read app state"))
}

/// Stores `value` as read by [`get_raw`]; `None` removes the key.
pub(crate) fn put_raw(
    connection: &Connection,
    key: &str,
    value: Option<&str>,
) -> Result<(), String> {
    match value {
        Some(text) => connection
            .execute(
                "INSERT INTO app_meta (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [key, text],
            )
            .map(|_| ())
            .map_err(sql_error("Failed to save app state")),
        None => remove(connection, key),
    }
}

/// Sets `key` unless it already has a value; the database wins over a file
/// that an older version wrote after the data moved.
pub(crate) fn set_if_absent<T: Serialize + ?Sized>(
    connection: &Connection,
    key: &str,
    value: &T,
) -> Result<(), String> {
    if !contains(connection, key)? {
        set(connection, key, value)?;
    }
    Ok(())
}

/// Moves the file at `path`, kept by older versions, into the database:
/// `import` gets its content inside a transaction, and the file is deleted
/// once that committed. A file that fails to import is left in place.
pub(crate) fn import_file(
    database: &Database,
    path: &Path,
    import: impl FnOnce(&Connection, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Failed to read '{}': {error}", path.display()))?;
    database
        .write(|transaction| import(transaction, &bytes))
        .map_err(|error| format!("Failed to import '{}': {error}", path.display()))?;
    std::fs::remove_file(path).map_err(|error| {
        format!(
            "Imported '{}' but could not delete it: {error}",
            path.display()
        )
    })
}

/// Parses a settings file's JSON for [`import_file`].
pub(crate) fn parse_file<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|error| format!("Invalid JSON: {error}"))
}

/// Moves `(field, key)` pairs out of the JSON object in `path` into
/// `app_meta`, for state older versions kept in their settings files. A key
/// already in the database wins, and the fields are removed from the file
/// afterwards, so this is a no-op once done.
pub(crate) fn move_file_fields(
    database: &Database,
    path: &Path,
    fields: &[(&str, &str)],
) -> Result<(), String> {
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
    database.write(|transaction| {
        for (key, value) in &moved {
            if !value.is_null() {
                set_if_absent(transaction, key, value)?;
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
                set_if_absent(connection, "a", &1_i64)?;
                set_if_absent(connection, "a", &2_i64)?;
                assert_eq!(get::<i64>(connection, "a")?, Some(1));
                Ok(())
            })
            .unwrap();
    }

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tterm-meta-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn import_file_moves_the_file_into_the_database_and_deletes_it() {
        let database = Database::open_in_memory().unwrap();
        let dir = temp_dir("import");
        let path = dir.join("settings.json");
        std::fs::write(&path, br#"{"a":1}"#).unwrap();

        import_file(&database, &path, |connection, bytes| {
            set_if_absent(connection, "settings", &parse_file::<Value>(bytes)?)
        })
        .unwrap();
        assert!(!path.exists());
        assert_eq!(
            database.read(|c| get::<Value>(c, "settings")).unwrap(),
            Some(serde_json::json!({"a": 1}))
        );

        // Nothing to do without the file.
        import_file(&database, &path, |_, _| panic!("no file to import")).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_file_that_fails_to_import_stays_and_nothing_is_written() {
        let database = Database::open_in_memory().unwrap();
        let dir = temp_dir("corrupt");
        let path = dir.join("settings.json");
        std::fs::write(&path, b"{ not json").unwrap();

        let error = import_file(&database, &path, |connection, bytes| {
            set(connection, "marker", &true)?;
            set(connection, "settings", &parse_file::<Value>(bytes)?)
        })
        .unwrap_err();
        assert!(error.contains("settings.json"), "{error}");
        assert!(path.exists());
        assert!(!database.read(|c| contains(c, "marker")).unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn move_file_fields_takes_the_fields_out_of_the_file() {
        let database = Database::open_in_memory().unwrap();
        database.write(|c| set(c, "kept", &1_i64)).unwrap();
        let dir = temp_dir("fields");
        let path = dir.join("config.json");
        std::fs::write(&path, br#"{"theme":"x","last":5,"old":9,"empty":null}"#).unwrap();

        let fields = [("last", "last"), ("old", "kept"), ("empty", "empty")];
        move_file_fields(&database, &path, &fields).unwrap();
        let file: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(file, serde_json::json!({"theme": "x"}));
        database
            .read(|c| {
                assert_eq!(get::<i64>(c, "last")?, Some(5));
                // The database wins over a value left in the file.
                assert_eq!(get::<i64>(c, "kept")?, Some(1));
                assert!(!contains(c, "empty")?);
                Ok(())
            })
            .unwrap();

        // Done once; the file is left alone afterwards.
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        move_file_fields(&database, &path, &fields).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            modified
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
