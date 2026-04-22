use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::config::ensure_config_dir;

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

fn sftp_directory_store_path() -> Result<PathBuf, String> {
    Ok(ensure_config_dir()?.join("sftp_directories.json"))
}

pub fn load_sftp_directory_store() -> Result<SftpDirectoryStore, String> {
    let path = sftp_directory_store_path()?;
    if !path.exists() {
        return Ok(SftpDirectoryStore::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read SFTP directory store: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse SFTP directory store: {}", e))
}

pub fn save_sftp_directory_store(store: &SftpDirectoryStore) -> Result<(), String> {
    let path = sftp_directory_store_path()?;
    let content = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize SFTP directory store: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("Failed to write SFTP directory store: {}", e))
}

pub fn get_last_directory(host: &str, port: u16, username: &str) -> Result<Option<String>, String> {
    let store = load_sftp_directory_store()?;
    Ok(store
        .entries
        .iter()
        .find(|entry| entry.host == host && entry.port == port && entry.username == username)
        .map(|entry| entry.last_path.clone()))
}

pub fn save_last_directory(
    host: &str,
    port: u16,
    username: &str,
    last_path: &str,
) -> Result<(), String> {
    let mut store = load_sftp_directory_store()?;

    if let Some(existing) = store
        .entries
        .iter_mut()
        .find(|entry| entry.host == host && entry.port == port && entry.username == username)
    {
        existing.last_path = last_path.to_string();
    } else {
        store.entries.push(SftpLastDirectory {
            host: host.to_string(),
            port,
            username: username.to_string(),
            last_path: last_path.to_string(),
        });
    }

    save_sftp_directory_store(&store)
}
