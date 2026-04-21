use crate::config::ensure_config_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LegacySshPasswordRecord {
    pub profile_name: String,
    pub password: String,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LegacySshPasswordStore {
    #[serde(default)]
    pub profiles: Vec<LegacySshPasswordRecord>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KnownHostRecord {
    #[serde(default)]
    pub profile_id: Option<String>,
    pub profile_name: String,
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
    pub trusted_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct KnownHostStore {
    #[serde(default)]
    pub entries: Vec<KnownHostRecord>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshHostKeyPromptPayload {
    pub request_id: String,
    pub profile_name: String,
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
    pub reason: String,
    pub known_fingerprint: Option<String>,
}

pub fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn legacy_password_store_path() -> Result<PathBuf, String> {
    Ok(ensure_config_dir()?.join("ssh_profiles.json"))
}

pub fn ssh_known_hosts_path() -> Result<PathBuf, String> {
    Ok(ensure_config_dir()?.join("ssh_known_hosts.json"))
}

pub fn load_legacy_password_store() -> Result<LegacySshPasswordStore, String> {
    let path = legacy_password_store_path()?;
    if !path.exists() {
        return Ok(LegacySshPasswordStore::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read SSH password store: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse SSH password store: {}", e))
}


pub fn remove_legacy_password_store() -> Result<(), String> {
    let path = legacy_password_store_path()?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("Failed to remove SSH password store: {}", e))?;
    }
    Ok(())
}

pub fn load_known_host_store() -> Result<KnownHostStore, String> {
    let path = ssh_known_hosts_path()?;
    if !path.exists() {
        return Ok(KnownHostStore::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read known hosts store: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse known hosts store: {}", e))
}

pub fn save_known_host_store(store: &KnownHostStore) -> Result<(), String> {
    let path = ssh_known_hosts_path()?;
    let content = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize known hosts store: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("Failed to write known hosts store: {}", e))
}

pub fn load_known_host(
    profile_name: &str,
    profile_id: Option<&str>,
) -> Result<Option<KnownHostRecord>, String> {
    let store = load_known_host_store()?;
    let profile_id = profile_id.map(str::trim).filter(|value| !value.is_empty());

    if let Some(profile_id) = profile_id {
        if let Some(entry) = store
            .entries
            .iter()
            .find(|entry| entry.profile_id.as_deref() == Some(profile_id))
            .cloned()
        {
            return Ok(Some(entry));
        }
    }

    let mut entry = store
        .entries
        .iter()
        .find(|entry| entry.profile_name == profile_name)
        .cloned();

    if let (Some(profile_id), Some(record)) = (profile_id, entry.as_mut()) {
        if record.profile_id.as_deref() != Some(profile_id) {
            record.profile_id = Some(profile_id.to_string());
            let _ = save_known_host_entry(record.clone());
        }
    }

    Ok(entry)
}

pub fn save_known_host_entry(entry: KnownHostRecord) -> Result<(), String> {
    let profile_id = entry.profile_id.as_deref();
    let mut store = load_known_host_store()?;
    if let Some(existing) = store
        .entries
        .iter_mut()
        .find(|existing| profile_id.is_some() && existing.profile_id.as_deref() == profile_id)
    {
        *existing = entry;
    } else if let Some(existing) = store
        .entries
        .iter_mut()
        .find(|existing| existing.profile_name == entry.profile_name)
    {
        *existing = entry;
    } else {
        store.entries.push(entry);
    }
    save_known_host_store(&store)
}
