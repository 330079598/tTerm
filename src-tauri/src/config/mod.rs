mod paths;

pub use paths::{ensure_config_dir, get_config_path, init_config_dir, legacy_config_path};

use serde::{Deserialize, Serialize};
use std::fs;
use sys_locale::get_locale;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub theme: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: u16,
    #[serde(default = "default_cursor_style")]
    pub cursor_style: String,
    #[serde(default = "default_terminal_shell")]
    pub terminal_shell: String,
    #[serde(default)]
    pub terminal_shell_custom_path: String,
    #[serde(default)]
    pub terminal_shell_custom_args: String,
    #[serde(default)]
    pub secret_vault_enabled: bool,
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: u32,
    #[serde(default = "default_startup_session_restore_mode")]
    pub startup_session_restore_mode: String,
    #[serde(default = "default_show_jump_host_connection_info")]
    pub show_jump_host_connection_info: bool,
    #[serde(default = "default_update_channel")]
    pub update_channel: String,
    #[serde(default = "default_auto_download_updates")]
    pub auto_download_updates: bool,
    #[serde(default = "default_update_check_frequency")]
    pub update_check_frequency: String,
}

fn normalize_language(locale: &str) -> String {
    let normalized_locale = locale.replace('_', "-").to_ascii_lowercase();

    if normalized_locale.starts_with("zh") {
        return "zh".to_string();
    }

    "en".to_string()
}

fn default_language() -> String {
    get_locale()
        .map(|locale| normalize_language(&locale))
        .unwrap_or_else(|| "en".to_string())
}

fn default_font_family() -> String {
    #[cfg(target_os = "macos")]
    return "Menlo, Monaco, monospace".to_string();
    #[cfg(target_os = "windows")]
    return "\"Cascadia Code\", Consolas, monospace".to_string();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return "\"DejaVu Sans Mono\", monospace".to_string();
}

fn default_font_size() -> u16 {
    14
}

fn default_cursor_style() -> String {
    "block".to_string()
}

fn default_terminal_shell() -> String {
    "auto".to_string()
}

fn default_scrollback_lines() -> u32 {
    10000
}

fn default_startup_session_restore_mode() -> String {
    "active".to_string()
}

fn default_show_jump_host_connection_info() -> bool {
    true
}

fn default_update_channel() -> String {
    "stable".to_string()
}

fn default_auto_download_updates() -> bool {
    true
}

fn default_update_check_frequency() -> String {
    "daily".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            language: default_language(),
            font_family: default_font_family(),
            font_size: default_font_size(),
            cursor_style: default_cursor_style(),
            terminal_shell: default_terminal_shell(),
            terminal_shell_custom_path: String::new(),
            terminal_shell_custom_args: String::new(),
            secret_vault_enabled: false,
            scrollback_lines: default_scrollback_lines(),
            startup_session_restore_mode: default_startup_session_restore_mode(),
            show_jump_host_connection_info: default_show_jump_host_connection_info(),
            update_channel: default_update_channel(),
            auto_download_updates: default_auto_download_updates(),
            update_check_frequency: default_update_check_frequency(),
        }
    }
}

fn config_file_path() -> Result<std::path::PathBuf, String> {
    Ok(get_config_path()?.join("config.json"))
}

pub fn load_config_file() -> Result<AppConfig, String> {
    let config_file = config_file_path()?;
    if !config_file.exists() {
        return Ok(AppConfig::default());
    }
    let content = fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
}

pub fn save_config_file(config: &AppConfig) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let config_file = config_dir.join("config.json");
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    fs::write(&config_file, content).map_err(|e| format!("Failed to write config file: {}", e))
}

#[tauri::command]
pub fn load_config() -> Result<AppConfig, String> {
    load_config_file()
}

#[tauri::command]
pub fn save_config(config: AppConfig) -> Result<(), String> {
    save_config_file(&config)
}
