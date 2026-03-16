use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppConfig {
    theme: String,
    language: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            language: "en".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SessionData {
    tabs: serde_json::Value,
    active_tab_id: Option<String>,
    last_saved: i64,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            tabs: serde_json::json!([]),
            active_tab_id: None,
            last_saved: 0,
        }
    }
}

fn get_config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "Failed to get HOME directory")?;
    let config_dir = PathBuf::from(home).join(".config").join("tterm");
    Ok(config_dir)
}

fn ensure_config_dir() -> Result<PathBuf, String> {
    let config_dir = get_config_path()?;
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    Ok(config_dir)
}

#[tauri::command]
fn load_config() -> Result<AppConfig, String> {
    let config_dir = get_config_path()?;
    let config_file = config_dir.join("config.json");
    
    if !config_file.exists() {
        return Ok(AppConfig::default());
    }
    
    let content = fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    
    let config: AppConfig = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;
    
    Ok(config)
}

#[tauri::command]
fn save_config(config: AppConfig) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let config_file = config_dir.join("config.json");
    
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    fs::write(&config_file, content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;
    
    Ok(())
}

#[tauri::command]
fn load_session() -> Result<SessionData, String> {
    let config_dir = get_config_path()?;
    let session_file = config_dir.join("session.json");
    
    if !session_file.exists() {
        return Ok(SessionData::default());
    }
    
    let content = fs::read_to_string(&session_file)
        .map_err(|e| format!("Failed to read session file: {}", e))?;
    
    let session: SessionData = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse session file: {}", e))?;
    
    // Check if session is expired (more than 7 days)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    
    let max_age = 7 * 24 * 60 * 60 * 1000; // 7 days in milliseconds
    if now - session.last_saved > max_age {
        // Session expired, return default
        return Ok(SessionData::default());
    }
    
    Ok(session)
}

#[tauri::command]
fn save_session(session: SessionData) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let session_file = config_dir.join("session.json");
    
    let content = serde_json::to_string_pretty(&session)
        .map_err(|e| format!("Failed to serialize session: {}", e))?;
    
    fs::write(&session_file, content)
        .map_err(|e| format!("Failed to write session file: {}", e))?;
    
    Ok(())
}

#[tauri::command]
fn clear_session() -> Result<(), String> {
    let config_dir = get_config_path()?;
    let session_file = config_dir.join("session.json");
    
    if session_file.exists() {
        fs::remove_file(&session_file)
            .map_err(|e| format!("Failed to delete session file: {}", e))?;
    }
    
    Ok(())
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .invoke_handler(tauri::generate_handler![
            greet, 
            load_config, 
            save_config,
            load_session,
            save_session,
            clear_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
