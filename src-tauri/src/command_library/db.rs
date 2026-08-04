use super::migrations;
use crate::config::ensure_config_dir;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

pub struct CommandDatabase {
    pub(super) connection: Mutex<Connection>,
}

impl CommandDatabase {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut connection = Connection::open(path).map_err(|error| {
            format!(
                "Failed to open command database '{}': {error}",
                path.display()
            )
        })?;
        configure(&connection)?;
        migrations::apply(&mut connection)?;
        restrict_file_permissions(path)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    #[cfg(test)]
    pub(super) fn open_in_memory() -> Result<Self, String> {
        let mut connection = Connection::open_in_memory()
            .map_err(|error| format!("Failed to open in-memory command database: {error}"))?;
        configure(&connection)?;
        migrations::apply(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
}

pub struct CommandLibraryState {
    database: Result<CommandDatabase, String>,
}

impl CommandLibraryState {
    pub fn initialize() -> Self {
        let database = ensure_config_dir()
            .map(|directory| directory.join("tterm.db"))
            .and_then(|path| CommandDatabase::open(&path));
        Self { database }
    }

    pub fn database(&self) -> Result<&CommandDatabase, String> {
        self.database.as_ref().map_err(Clone::clone)
    }

    pub fn initialization_error(&self) -> Option<&str> {
        self.database.as_ref().err().map(String::as_str)
    }
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
        .map_err(|error| format!("Failed to configure command database: {error}"))
}

#[cfg(unix)]
fn restrict_file_permissions(path: &Path) -> Result<(), String> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!(
            "Failed to restrict command database permissions '{}': {error}",
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
        let database = CommandDatabase::open_in_memory().expect("open database");
        let connection = database.connection.lock().expect("lock database");
        let enabled: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("read pragma");
        assert_eq!(enabled, 1);
    }
}
