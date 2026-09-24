//! The app's SQLite database (`tterm.db` in the config directory).
//!
//! One connection is shared by every store. Stores keep their queries in
//! functions that take a `&Connection` so tests can run them against an
//! in-memory database, and reach the app database through [`get`].

mod legacy_import;
mod migrations;

pub(crate) use legacy_import::IMPORTED_FILES as LEGACY_JSON_FILES;

use crate::config::ensure_config_dir;
use rusqlite::{Connection, Transaction};
use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

const DATABASE_FILE: &str = "tterm.db";

static DATABASE: OnceLock<Result<Database, String>> = OnceLock::new();
/// Set when the JSON import failed; the database then refuses to be used so
/// nothing is written that a retried import would overwrite.
static IMPORT_ERROR: OnceLock<String> = OnceLock::new();

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut connection = Connection::open(path)
            .map_err(|error| format!("Failed to open database '{}': {error}", path.display()))?;
        configure(&connection)?;
        migrations::apply(&mut connection)?;
        restrict_file_permissions(path)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self, String> {
        let mut connection = Connection::open_in_memory()
            .map_err(|error| format!("Failed to open in-memory database: {error}"))?;
        configure(&connection)?;
        migrations::apply(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.connection
            .lock()
            .map_err(|_| "Database lock was poisoned".to_string())
    }

    /// Runs `f` on the shared connection.
    pub(crate) fn read<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let connection = self.lock()?;
        f(&connection)
    }

    /// Runs `f` in a transaction that commits only when `f` succeeds.
    pub(crate) fn write<T>(
        &self,
        f: impl FnOnce(&Transaction<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("Failed to start database transaction: {error}"))?;
        let value = f(&transaction)?;
        transaction
            .commit()
            .map_err(|error| format!("Failed to commit database transaction: {error}"))?;
        Ok(value)
    }
}

/// Opens the app database. Called once during setup, after the config
/// directory is initialized; the outcome is kept so later calls to [`get`]
/// report the same error instead of retrying.
pub fn init() -> Result<&'static Database, String> {
    DATABASE
        .get_or_init(|| {
            ensure_config_dir()
                .map(|directory| directory.join(DATABASE_FILE))
                .and_then(|path| Database::open(&path))
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// Whether the legacy JSON files have been imported. False when the database
/// is unavailable.
pub(crate) fn legacy_json_imported() -> bool {
    init()
        .and_then(legacy_import::legacy_json_imported)
        .unwrap_or(false)
}

/// Imports the JSON files that held app data before the database, once.
pub(crate) fn import_legacy_json_files() -> Result<(), String> {
    let database = init()?;
    let result = ensure_config_dir()
        .and_then(|directory| legacy_import::import_legacy_json_files(database, &directory));
    match result {
        Ok(_) => Ok(()),
        Err(error) => {
            let _ = IMPORT_ERROR.set(error.clone());
            Err(error)
        }
    }
}

pub fn get() -> Result<&'static Database, String> {
    if let Some(error) = IMPORT_ERROR.get() {
        return Err(error.clone());
    }
    match DATABASE.get() {
        Some(database) => database.as_ref().map_err(Clone::clone),
        None => Err("Database is not initialized".to_string()),
    }
}

pub(crate) fn read<T>(f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    get()?.read(f)
}

pub(crate) fn write<T>(f: impl FnOnce(&Transaction<'_>) -> Result<T, String>) -> Result<T, String> {
    get()?.write(f)
}

pub(crate) fn sql_error(context: &str) -> impl Fn(rusqlite::Error) -> String + '_ {
    move |error| format!("{context}: {error}")
}

fn configure(connection: &Connection) -> Result<(), String> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("Failed to configure SQLite busy timeout: {error}"))?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;\
             PRAGMA journal_mode = WAL;\
             PRAGMA synchronous = NORMAL;",
        )
        .map_err(|error| format!("Failed to configure database: {error}"))
}

#[cfg(unix)]
fn restrict_file_permissions(path: &Path) -> Result<(), String> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!(
            "Failed to restrict database permissions '{}': {error}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn restrict_file_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_enables_foreign_keys() {
        let database = Database::open_in_memory().expect("open database");
        let enabled: i64 = database
            .read(|connection| {
                connection
                    .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
                    .map_err(|error| error.to_string())
            })
            .expect("read pragma");
        assert_eq!(enabled, 1);
    }

    #[test]
    fn failed_write_rolls_back() {
        let database = Database::open_in_memory().expect("open database");
        let result: Result<(), String> = database.write(|transaction| {
            transaction
                .execute("INSERT INTO profile_groups (name) VALUES ('ops')", [])
                .map_err(|error| error.to_string())?;
            Err("boom".to_string())
        });
        assert!(result.is_err());
        let count: i64 = database
            .read(|connection| {
                connection
                    .query_row("SELECT COUNT(*) FROM profile_groups", [], |row| row.get(0))
                    .map_err(|error| error.to_string())
            })
            .expect("count groups");
        assert_eq!(count, 0);
    }
}
