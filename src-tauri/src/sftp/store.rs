use crate::db::sql_error;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SftpLastDirectory {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub last_path: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SftpDirectoryStore {
    #[serde(default)]
    pub entries: Vec<SftpLastDirectory>,
}

pub(crate) fn list_sftp_directories(connection: &Connection) -> Result<SftpDirectoryStore, String> {
    let mut statement = connection
        .prepare("SELECT host, port, username, last_path FROM sftp_last_directories ORDER BY rowid")
        .map_err(sql_error("Failed to read SFTP directories"))?;
    let entries = statement
        .query_map([], |row| {
            Ok(SftpLastDirectory {
                host: row.get(0)?,
                port: row.get(1)?,
                username: row.get(2)?,
                last_path: row.get(3)?,
            })
        })
        .and_then(Iterator::collect)
        .map_err(sql_error("Failed to read SFTP directories"))?;
    Ok(SftpDirectoryStore { entries })
}

fn upsert_sftp_directory(connection: &Connection, entry: &SftpLastDirectory) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO sftp_last_directories (host, port, username, last_path) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(host, port, username) DO UPDATE SET last_path = excluded.last_path",
            params![entry.host, entry.port, entry.username, entry.last_path],
        )
        .map_err(sql_error("Failed to save SFTP directory"))?;
    Ok(())
}

pub(crate) fn replace_sftp_directories(
    connection: &Connection,
    store: &SftpDirectoryStore,
) -> Result<(), String> {
    connection
        .execute("DELETE FROM sftp_last_directories", [])
        .map_err(sql_error("Failed to clear SFTP directories"))?;
    for entry in &store.entries {
        upsert_sftp_directory(connection, entry)?;
    }
    Ok(())
}

pub fn get_last_directory(host: &str, port: u16, username: &str) -> Result<Option<String>, String> {
    crate::db::read(|connection| {
        connection
            .query_row(
                "SELECT last_path FROM sftp_last_directories \
                 WHERE host = ?1 AND port = ?2 AND username = ?3",
                params![host, port, username],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_error("Failed to read SFTP directory"))
    })
}

pub fn save_last_directory(
    host: &str,
    port: u16,
    username: &str,
    last_path: &str,
) -> Result<(), String> {
    let entry = SftpLastDirectory {
        host: host.to_string(),
        port,
        username: username.to_string(),
        last_path: last_path.to_string(),
    };
    crate::db::write(|transaction| upsert_sftp_directory(transaction, &entry))
}
