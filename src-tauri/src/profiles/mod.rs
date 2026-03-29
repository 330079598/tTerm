use crate::config::{ensure_config_dir, get_config_path};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group: String,
    pub connection_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default)]
    pub remember_password: bool,
    pub auth_method: Option<String>,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
    #[serde(default)]
    pub reconnect: bool,
    #[serde(default = "default_reconnect_delay")]
    pub reconnect_delay_secs: u32,
    #[serde(default = "default_reconnect_max_delay")]
    pub reconnect_max_delay_secs: u32,
    #[serde(default = "default_reconnect_max_retries")]
    pub reconnect_max_retries: u32,
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval_secs: u32,
    #[serde(default = "default_keepalive_count")]
    pub keepalive_count_max: u32,
}

fn default_reconnect_delay() -> u32 {
    5
}
fn default_reconnect_max_delay() -> u32 {
    60
}
fn default_reconnect_max_retries() -> u32 {
    10
}
fn default_keepalive_interval() -> u32 {
    30
}
fn default_keepalive_count() -> u32 {
    3
}

fn load_profiles_from_disk() -> Result<Vec<SavedProfile>, String> {
    let config_dir = get_config_path()?;
    let profiles_file = config_dir.join("profiles.json");
    if !profiles_file.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&profiles_file)
        .map_err(|e| format!("Failed to read profiles file: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse profiles: {}", e))
}

fn write_profiles_to_disk(profiles: &Vec<SavedProfile>) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let profiles_file = config_dir.join("profiles.json");
    let content = serde_json::to_string_pretty(profiles)
        .map_err(|e| format!("Failed to serialize profiles: {}", e))?;
    fs::write(&profiles_file, content).map_err(|e| format!("Failed to write profiles file: {}", e))
}

#[tauri::command]
pub fn list_profiles() -> Result<Vec<SavedProfile>, String> {
    load_profiles_from_disk()
}

#[tauri::command]
pub fn save_profile(profile: SavedProfile) -> Result<(), String> {
    let mut profiles = load_profiles_from_disk()?;
    if let Some(pos) = profiles.iter().position(|p| p.id == profile.id) {
        profiles[pos] = profile;
    } else {
        profiles.push(profile);
    }
    write_profiles_to_disk(&profiles)
}

#[tauri::command]
pub fn delete_profile(id: String) -> Result<(), String> {
    let mut profiles = load_profiles_from_disk()?;
    profiles.retain(|p| p.id != id);
    write_profiles_to_disk(&profiles)
}
