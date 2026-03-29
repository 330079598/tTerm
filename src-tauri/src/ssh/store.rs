use crate::config::ensure_config_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SshPasswordRecord {
    pub profile_name: String,
    pub password: String,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SshPasswordStore {
    #[serde(default)]
    pub profiles: Vec<SshPasswordRecord>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KnownHostRecord {
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

pub fn ssh_password_store_path() -> Result<PathBuf, String> {
    Ok(ensure_config_dir()?.join("ssh_profiles.json"))
}

pub fn ssh_known_hosts_path() -> Result<PathBuf, String> {
    Ok(ensure_config_dir()?.join("ssh_known_hosts.json"))
}

pub fn load_password_store() -> Result<SshPasswordStore, String> {
    let path = ssh_password_store_path()?;
    if !path.exists() {
        return Ok(SshPasswordStore::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read SSH password store: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse SSH password store: {}", e))
}

pub fn save_password_store(store: &SshPasswordStore) -> Result<(), String> {
    let path = ssh_password_store_path()?;
    let content = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize SSH password store: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("Failed to write SSH password store: {}", e))
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

pub fn load_password_for_profile(profile_name: &str) -> Result<Option<String>, String> {
    let store = load_password_store()?;
    Ok(store
        .profiles
        .iter()
        .find(|p| p.profile_name == profile_name)
        .map(|p| p.password.clone()))
}

pub fn save_password_for_profile(profile_name: &str, password: &str) -> Result<(), String> {
    let mut store = load_password_store()?;
    if let Some(existing) = store
        .profiles
        .iter_mut()
        .find(|p| p.profile_name == profile_name)
    {
        existing.password = password.to_string();
        existing.updated_at = now_unix_ms();
    } else {
        store.profiles.push(SshPasswordRecord {
            profile_name: profile_name.to_string(),
            password: password.to_string(),
            updated_at: now_unix_ms(),
        });
    }
    save_password_store(&store)
}

pub fn load_known_host_by_profile(profile_name: &str) -> Result<Option<KnownHostRecord>, String> {
    let store = load_known_host_store()?;
    Ok(store
        .entries
        .iter()
        .find(|e| e.profile_name == profile_name)
        .cloned())
}

pub fn save_known_host_entry(entry: KnownHostRecord) -> Result<(), String> {
    let mut store = load_known_host_store()?;
    if let Some(existing) = store
        .entries
        .iter_mut()
        .find(|e| e.profile_name == entry.profile_name)
    {
        *existing = entry;
    } else {
        store.entries.push(entry);
    }
    save_known_host_store(&store)
}
