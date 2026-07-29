mod atomic;
mod paths;

pub use atomic::{atomic_write, atomic_write_private};
pub use paths::{ensure_config_dir, get_config_path, init_config_dir, legacy_config_path};

use serde::{Deserialize, Deserializer, Serialize};
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
    #[serde(
        default = "default_ui_scale_percent",
        deserialize_with = "deserialize_ui_scale_percent"
    )]
    pub ui_scale_percent: u16,
    #[serde(default = "default_cursor_style")]
    pub cursor_style: String,
    #[serde(default = "default_terminal_shell")]
    pub terminal_shell: String,
    #[serde(default)]
    pub terminal_shell_custom_path: String,
    #[serde(default)]
    pub terminal_shell_custom_args: String,
    #[serde(default = "default_secret_vault_enabled")]
    pub secret_vault_enabled: bool,
    #[serde(default = "default_secret_storage_mode")]
    pub secret_storage_mode: String,
    #[serde(default)]
    pub prompt_unlock_vault_on_startup: bool,
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: u32,
    #[serde(default = "default_terminal_padding_left_px")]
    pub terminal_padding_left_px: u16,
    #[serde(default)]
    pub terminal_padding_right_px: u16,
    #[serde(default)]
    pub terminal_padding_bottom_px: u16,
    #[serde(default = "default_startup_session_restore_mode")]
    pub startup_session_restore_mode: String,
    #[serde(default = "default_show_jump_host_connection_info")]
    pub show_jump_host_connection_info: bool,
    #[serde(default)]
    pub sftp_paste_upload_enabled: bool,
    #[serde(default = "default_monitor_refresh_interval_secs")]
    pub monitor_refresh_interval_secs: u16,
    #[serde(default = "default_update_channel")]
    pub update_channel: String,
    #[serde(default = "default_auto_download_updates")]
    pub auto_download_updates: bool,
    #[serde(default = "default_update_check_frequency")]
    pub update_check_frequency: String,
    #[serde(default)]
    pub last_update_check_at: Option<i64>,
    #[serde(default)]
    pub collapsed_profile_group_keys: Vec<String>,
    #[serde(
        default = "default_tab_width_mode",
        deserialize_with = "deserialize_tab_width_mode"
    )]
    pub tab_width_mode: String,
    #[serde(
        default = "default_tab_standard_width",
        deserialize_with = "deserialize_tab_standard_width"
    )]
    pub tab_standard_width: u16,
    #[serde(default)]
    pub terminal_log_enabled: bool,
    #[serde(default)]
    pub terminal_log_directory: String,
    #[serde(default = "default_terminal_log_format")]
    pub terminal_log_format: String,
    #[serde(default = "default_terminal_log_name_template")]
    pub terminal_log_name_template: String,
    #[serde(default = "default_terminal_log_max_file_size_mb")]
    pub terminal_log_max_file_size_mb: u32,
    #[serde(default)]
    pub terminal_log_compress: bool,
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

fn default_ui_scale_percent() -> u16 {
    100
}

fn deserialize_ui_scale_percent<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(((value / 10.0).round() * 10.0).clamp(80.0, 200.0) as u16)
}

fn default_cursor_style() -> String {
    "block".to_string()
}

fn default_terminal_shell() -> String {
    "auto".to_string()
}

fn default_secret_storage_mode() -> String {
    if cfg!(target_os = "windows") {
        "system".to_string()
    } else {
        "hybrid".to_string()
    }
}

fn default_secret_vault_enabled() -> bool {
    true
}

fn default_scrollback_lines() -> u32 {
    10000
}

fn default_terminal_padding_left_px() -> u16 {
    6
}

fn default_startup_session_restore_mode() -> String {
    "active".to_string()
}

fn default_show_jump_host_connection_info() -> bool {
    true
}

fn default_monitor_refresh_interval_secs() -> u16 {
    5
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

fn default_tab_width_mode() -> String {
    "adaptive".to_string()
}

fn default_tab_standard_width() -> u16 {
    120
}

fn default_terminal_log_format() -> String {
    "both".to_string()
}

fn default_terminal_log_name_template() -> String {
    "{profile}-{host}-{yyyyMMdd-HHmmss}-{sessionId}".to_string()
}

fn default_terminal_log_max_file_size_mb() -> u32 {
    50
}

fn deserialize_tab_width_mode<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let mode = String::deserialize(deserializer)?;
    Ok(if mode == "standard" {
        mode
    } else {
        default_tab_width_mode()
    })
}

fn deserialize_tab_standard_width<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let width = f64::deserialize(deserializer)?;
    Ok(width.round().clamp(80.0, 300.0) as u16)
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            language: default_language(),
            font_family: default_font_family(),
            font_size: default_font_size(),
            ui_scale_percent: default_ui_scale_percent(),
            cursor_style: default_cursor_style(),
            terminal_shell: default_terminal_shell(),
            terminal_shell_custom_path: String::new(),
            terminal_shell_custom_args: String::new(),
            secret_vault_enabled: default_secret_vault_enabled(),
            secret_storage_mode: default_secret_storage_mode(),
            prompt_unlock_vault_on_startup: false,
            scrollback_lines: default_scrollback_lines(),
            terminal_padding_left_px: default_terminal_padding_left_px(),
            terminal_padding_right_px: 0,
            terminal_padding_bottom_px: 0,
            startup_session_restore_mode: default_startup_session_restore_mode(),
            show_jump_host_connection_info: default_show_jump_host_connection_info(),
            sftp_paste_upload_enabled: false,
            monitor_refresh_interval_secs: default_monitor_refresh_interval_secs(),
            update_channel: default_update_channel(),
            auto_download_updates: default_auto_download_updates(),
            update_check_frequency: default_update_check_frequency(),
            last_update_check_at: None,
            collapsed_profile_group_keys: Vec::new(),
            tab_width_mode: default_tab_width_mode(),
            tab_standard_width: default_tab_standard_width(),
            terminal_log_enabled: false,
            terminal_log_directory: String::new(),
            terminal_log_format: default_terminal_log_format(),
            terminal_log_name_template: default_terminal_log_name_template(),
            terminal_log_max_file_size_mb: default_terminal_log_max_file_size_mb(),
            terminal_log_compress: false,
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
    atomic_write(&config_file, content)
}

#[tauri::command]
pub fn load_config() -> Result<AppConfig, String> {
    load_config_file()
}

#[tauri::command]
pub fn save_config(
    app: tauri::AppHandle,
    config: AppConfig,
    log_state: tauri::State<'_, crate::session_log::SessionLogState>,
) -> Result<(), String> {
    log_state.validate_config(&config)?;
    save_config_file(&config)?;
    log_state.apply_config(&app, &config)
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn legacy_config_uses_adaptive_tab_width_defaults() {
        let config: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();

        assert_eq!(config.tab_width_mode, "adaptive");
        assert_eq!(config.tab_standard_width, 120);
        assert_eq!(config.ui_scale_percent, 100);
        assert!(!config.terminal_log_enabled);
        assert_eq!(config.terminal_log_format, "both");
        assert_eq!(config.terminal_log_max_file_size_mb, 50);
    }

    #[test]
    fn explicit_tab_width_config_is_deserialized() {
        let config: AppConfig = serde_json::from_str(
            r#"{"theme":"default","tab_width_mode":"standard","tab_standard_width":180}"#,
        )
        .unwrap();

        assert_eq!(config.tab_width_mode, "standard");
        assert_eq!(config.tab_standard_width, 180);
    }

    #[test]
    fn invalid_tab_width_config_is_normalized() {
        let config: AppConfig = serde_json::from_str(
            r#"{"theme":"default","tab_width_mode":"wide","tab_standard_width":79.6}"#,
        )
        .unwrap();

        assert_eq!(config.tab_width_mode, "adaptive");
        assert_eq!(config.tab_standard_width, 80);
    }

    #[test]
    fn ui_scale_config_is_rounded_and_clamped() {
        let rounded: AppConfig =
            serde_json::from_str(r#"{"theme":"default","ui_scale_percent":146}"#).unwrap();
        let minimum: AppConfig =
            serde_json::from_str(r#"{"theme":"default","ui_scale_percent":40}"#).unwrap();
        let maximum: AppConfig =
            serde_json::from_str(r#"{"theme":"default","ui_scale_percent":260}"#).unwrap();

        assert_eq!(rounded.ui_scale_percent, 150);
        assert_eq!(minimum.ui_scale_percent, 80);
        assert_eq!(maximum.ui_scale_percent, 200);
    }
}
