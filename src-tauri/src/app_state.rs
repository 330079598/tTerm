//! State the app keeps for itself rather than settings the user chose, so it
//! stays out of `config.json` (and so out of backups and sync).

use crate::core::blocking::run_blocking;
use crate::db::meta;
use serde::{Deserialize, Serialize};

const LAST_UPDATE_CHECK_AT: &str = "state.last_update_check_at";
const COLLAPSED_PROFILE_GROUP_KEYS: &str = "state.collapsed_profile_group_keys";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppState {
    pub last_update_check_at: Option<i64>,
    pub collapsed_profile_group_keys: Vec<String>,
}

pub(crate) fn load(connection: &rusqlite::Connection) -> Result<AppState, String> {
    Ok(AppState {
        last_update_check_at: meta::get(connection, LAST_UPDATE_CHECK_AT)?,
        collapsed_profile_group_keys: meta::get(connection, COLLAPSED_PROFILE_GROUP_KEYS)?
            .unwrap_or_default(),
    })
}

fn save(connection: &rusqlite::Connection, state: &AppState) -> Result<(), String> {
    match state.last_update_check_at {
        Some(at) => meta::set(connection, LAST_UPDATE_CHECK_AT, &at)?,
        None => meta::remove(connection, LAST_UPDATE_CHECK_AT)?,
    }
    meta::set(
        connection,
        COLLAPSED_PROFILE_GROUP_KEYS,
        &state.collapsed_profile_group_keys,
    )
}

/// Moves the state older versions kept in `config.json` into the database.
pub(crate) fn import_from_config_file(
    database: &crate::db::Database,
    config_dir: &std::path::Path,
) -> Result<(), String> {
    meta::move_file_fields(
        database,
        &config_dir.join("config.json"),
        &[
            ("last_update_check_at", LAST_UPDATE_CHECK_AT),
            ("collapsed_profile_group_keys", COLLAPSED_PROFILE_GROUP_KEYS),
        ],
    )
}

#[tauri::command]
pub async fn load_app_state() -> Result<AppState, String> {
    run_blocking(|| crate::db::read(load)).await
}

#[tauri::command]
pub async fn save_app_state(state: AppState) -> Result<(), String> {
    run_blocking(move || crate::db::write(|transaction| save(transaction, &state))).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn state_round_trips() {
        let database = Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                assert_eq!(load(connection)?, AppState::default());
                let state = AppState {
                    last_update_check_at: Some(5),
                    collapsed_profile_group_keys: vec!["ops".to_string()],
                };
                save(connection, &state)?;
                assert_eq!(load(connection)?, state);
                save(connection, &AppState::default())?;
                assert_eq!(load(connection)?, AppState::default());
                Ok(())
            })
            .unwrap();
    }
}
