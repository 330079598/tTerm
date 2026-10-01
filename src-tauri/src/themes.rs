//! Custom themes and edited preset themes. The web view defines a theme's
//! shape; each row keeps one theme as JSON, in display order.

use crate::core::blocking::run_blocking;
use crate::db::{meta, sql_error};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::HashSet;

/// Set once the themes the web view kept in localStorage were taken over.
/// Until then the table may be missing themes, so sync leaves themes alone.
const IMPORTED_KEY: &str = "custom_themes.imported";

/// Themes in display order.
pub(crate) fn list_themes(connection: &Connection) -> Result<Vec<Value>, String> {
    let mut statement = connection
        .prepare("SELECT id, data FROM custom_themes ORDER BY position, rowid")
        .map_err(sql_error("Failed to read themes"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .map_err(sql_error("Failed to read themes"))?;
    rows.into_iter()
        .map(|(id, data)| {
            serde_json::from_str(&data)
                .map_err(|error| format!("Failed to parse theme '{id}': {error}"))
        })
        .collect()
}

/// Replaces every theme, keeping the order given. Unchanged themes keep
/// their `updated_at`.
pub(crate) fn replace_themes(connection: &Connection, themes: &[Value]) -> Result<(), String> {
    let mut ids = HashSet::new();
    for theme in themes {
        let id = theme_id(theme)?;
        if !ids.insert(id) {
            return Err(format!("Theme '{id}' appears more than once."));
        }
    }
    let existing = {
        let mut statement = connection
            .prepare("SELECT id FROM custom_themes")
            .map_err(sql_error("Failed to read themes"))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(sql_error("Failed to read themes"))?;
        rows
    };
    for id in existing.iter().filter(|id| !ids.contains(id.as_str())) {
        delete_theme(connection, id)?;
    }
    for (position, theme) in themes.iter().enumerate() {
        upsert_theme(connection, theme, position as i64)?;
    }
    Ok(())
}

pub(crate) fn upsert_theme(
    connection: &Connection,
    theme: &Value,
    position: i64,
) -> Result<(), String> {
    let id = theme_id(theme)?;
    let data =
        serde_json::to_string(theme).map_err(|error| format!("Failed to encode theme: {error}"))?;
    connection
        .execute(
            "INSERT INTO custom_themes (id, position, data) VALUES (?1, ?2, ?3) \
             ON CONFLICT(id) DO UPDATE SET position = excluded.position, data = excluded.data",
            params![id, position, data],
        )
        .map_err(sql_error("Failed to save theme"))?;
    Ok(())
}

pub(crate) fn delete_theme(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM custom_themes WHERE id = ?1", [id])
        .map_err(sql_error("Failed to delete theme"))?;
    Ok(())
}

pub(crate) fn themes_imported(connection: &Connection) -> Result<bool, String> {
    meta::contains(connection, IMPORTED_KEY)
}

pub(crate) fn mark_themes_imported(connection: &Connection) -> Result<(), String> {
    if !themes_imported(connection)? {
        meta::set(
            connection,
            IMPORTED_KEY,
            &chrono::Utc::now().timestamp_millis(),
        )?;
    }
    Ok(())
}

fn theme_id(theme: &Value) -> Result<&str, String> {
    theme
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "A theme has no id.".to_string())
}

/// The saved themes. On the first call, `legacy` (the list the web view
/// kept in localStorage) is taken over when given.
#[tauri::command]
pub async fn load_custom_themes(legacy: Option<Vec<Value>>) -> Result<Vec<Value>, String> {
    run_blocking(move || {
        crate::db::write(|transaction| {
            if !themes_imported(transaction)? {
                if let Some(themes) = legacy.as_deref() {
                    let valid: Vec<Value> = themes
                        .iter()
                        .filter(|theme| theme_id(theme).is_ok())
                        .cloned()
                        .collect();
                    replace_themes(transaction, &dedupe(valid))?;
                }
                mark_themes_imported(transaction)?;
            }
            list_themes(transaction)
        })
    })
    .await
}

#[tauri::command]
pub async fn save_custom_themes(themes: Vec<Value>) -> Result<(), String> {
    run_blocking(move || {
        crate::db::write(|transaction| {
            replace_themes(transaction, &themes)?;
            mark_themes_imported(transaction)
        })
    })
    .await
}

/// Keeps the last theme of each id, where the first one stood.
fn dedupe(themes: Vec<Value>) -> Vec<Value> {
    let mut result: Vec<Value> = Vec::new();
    for theme in themes {
        let id = theme_id(&theme).map(str::to_string);
        match result
            .iter_mut()
            .find(|existing| theme_id(existing).map(str::to_string) == id)
        {
            Some(existing) => *existing = theme,
            None => result.push(theme),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use serde_json::json;

    fn updated_at(connection: &Connection, id: &str) -> Option<i64> {
        connection
            .query_row(
                "SELECT updated_at FROM custom_themes WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn replace_keeps_order_and_touches_only_changed_themes() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                let a = json!({"id": "a", "name": "A"});
                let b = json!({"id": "b", "name": "B"});
                replace_themes(connection, &[a.clone(), b.clone()])?;
                connection
                    .execute("UPDATE custom_themes SET updated_at = 1", [])
                    .unwrap();

                let b2 = json!({"id": "b", "name": "B2"});
                let c = json!({"id": "c", "name": "C"});
                replace_themes(connection, &[b2.clone(), a.clone(), c.clone()])?;
                assert_eq!(list_themes(connection)?, vec![b2, a, c]);
                assert_ne!(updated_at(connection, "b"), Some(1));
                // `a` moved, which counts as a change for sync.
                assert_ne!(updated_at(connection, "a"), Some(1));

                connection
                    .execute("UPDATE custom_themes SET updated_at = 1", [])
                    .unwrap();
                let unchanged = list_themes(connection)?;
                replace_themes(connection, &unchanged)?;
                assert_eq!(updated_at(connection, "a"), Some(1));

                replace_themes(connection, &unchanged[..1])?;
                assert_eq!(list_themes(connection)?.len(), 1);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn replace_rejects_themes_without_id_or_twice() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                assert!(replace_themes(connection, &[json!({"name": "x"})]).is_err());
                assert!(
                    replace_themes(connection, &[json!({"id": "a"}), json!({"id": "a"})]).is_err()
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn dedupe_keeps_the_last_copy_in_the_first_place() {
        let themes = vec![
            json!({"id": "a", "v": 1}),
            json!({"id": "b"}),
            json!({"id": "a", "v": 2}),
        ];
        assert_eq!(
            dedupe(themes),
            vec![json!({"id": "a", "v": 2}), json!({"id": "b"})]
        );
    }
}
