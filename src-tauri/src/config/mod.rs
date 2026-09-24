mod atomic;
mod paths;

pub use atomic::{atomic_write, atomic_write_private};
pub use paths::{ensure_config_dir, get_config_path, init_config_dir, legacy_config_path};

use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::sync::RwLock;
use std::time::SystemTime;
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
    #[serde(default = "default_terminal_renderer")]
    pub terminal_renderer: String,
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
    #[serde(default = "default_monitor_visible_metrics")]
    pub monitor_visible_metrics: Vec<String>,
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
    #[serde(default = "default_sftp_transfer_parallelism")]
    pub sftp_transfer_parallelism: u16,
    /// Automatically re-establish dropped SSH sessions with capped backoff.
    #[serde(default = "default_reconnect_enabled")]
    pub reconnect_enabled: bool,
    /// How many automatic reconnect attempts to make before giving up.
    #[serde(default = "default_reconnect_max_attempts")]
    pub reconnect_max_attempts: u32,
    /// Passively watch interactive shell sessions for a ZMODEM (`rz`/`sz`)
    /// invite and auto-start the transfer, mirroring SecureCRT/Xshell.
    #[serde(default = "default_zmodem_auto_detect_enabled")]
    pub zmodem_auto_detect_enabled: bool,
    /// Where auto-detected ZMODEM downloads are saved; empty resolves to the
    /// platform Downloads directory at time of use.
    #[serde(default)]
    pub zmodem_download_directory: String,
    #[serde(default = "default_keymap")]
    pub keymap: KeymapConfig,
}

/// User-configurable keyboard shortcut overrides. `bindings` maps action ids
/// to their chord serializations; `None` entries mean "unbound", missing
/// entries fall back to the tTerm defaults on the frontend.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeymapConfig {
    #[serde(default, deserialize_with = "deserialize_keymap_bindings")]
    pub bindings: std::collections::BTreeMap<String, Option<Vec<String>>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum KeymapBindingValue {
    One(String),
    Many(Vec<String>),
}

fn deserialize_keymap_bindings<'de, D>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<String, Option<Vec<String>>>, D::Error>
where
    D: Deserializer<'de>,
{
    let bindings = std::collections::BTreeMap::<String, Option<KeymapBindingValue>>::deserialize(
        deserializer,
    )?;
    Ok(bindings
        .into_iter()
        .map(|(action, value)| {
            let chords = value.map(|binding| match binding {
                KeymapBindingValue::One(chord) => vec![chord],
                KeymapBindingValue::Many(chords) => chords,
            });
            (action, chords)
        })
        .collect())
}

impl Default for KeymapConfig {
    fn default() -> Self {
        default_keymap()
    }
}

fn default_keymap() -> KeymapConfig {
    KeymapConfig {
        bindings: std::collections::BTreeMap::new(),
    }
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

/// Older versions' default. It tells the one-time password migration where
/// passwords were kept; afterwards `hybrid` means the same as `system`.
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

fn default_terminal_renderer() -> String {
    if cfg!(target_os = "macos") {
        "canvas".to_string()
    } else {
        "webgl".to_string()
    }
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

fn default_monitor_visible_metrics() -> Vec<String> {
    ["cpu", "memory", "network", "ip", "latency", "disk"]
        .into_iter()
        .map(str::to_string)
        .collect()
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

fn default_sftp_transfer_parallelism() -> u16 {
    4
}

fn default_reconnect_enabled() -> bool {
    true
}

fn default_reconnect_max_attempts() -> u32 {
    5
}

fn default_zmodem_auto_detect_enabled() -> bool {
    true
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
            terminal_renderer: default_terminal_renderer(),
            terminal_padding_left_px: default_terminal_padding_left_px(),
            terminal_padding_right_px: 0,
            terminal_padding_bottom_px: 0,
            startup_session_restore_mode: default_startup_session_restore_mode(),
            show_jump_host_connection_info: default_show_jump_host_connection_info(),
            sftp_paste_upload_enabled: false,
            monitor_refresh_interval_secs: default_monitor_refresh_interval_secs(),
            monitor_visible_metrics: default_monitor_visible_metrics(),
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
            sftp_transfer_parallelism: default_sftp_transfer_parallelism(),
            reconnect_enabled: default_reconnect_enabled(),
            reconnect_max_attempts: default_reconnect_max_attempts(),
            zmodem_auto_detect_enabled: default_zmodem_auto_detect_enabled(),
            zmodem_download_directory: String::new(),
            keymap: default_keymap(),
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

/// Resolved reconnect settings, cached against the config file's mtime: every
/// session normalizes its connection on create, and re-reading + re-parsing the
/// whole config from disk each time would add needless disk I/O to connect.
static RECONNECT_SETTINGS_CACHE: RwLock<Option<(Option<SystemTime>, bool, u32)>> =
    RwLock::new(None);

/// Cache lookup: a hit requires the whole mtime token — including `None`
/// ("config file absent") — to match, so a machine running on defaults still
/// hits the cache instead of stat-ing the disk on every connect.
fn reconnect_settings_cache_hit(
    cached: Option<(Option<SystemTime>, bool, u32)>,
    mtime: Option<SystemTime>,
) -> Option<(bool, u32)> {
    cached
        .filter(|(cached_mtime, _, _)| *cached_mtime == mtime)
        .map(|(_, enabled, max_attempts)| (enabled, max_attempts))
}

/// Resolve the automatic SSH reconnect settings (enabled, max attempts).
/// Falls back to the built-in defaults when the config file is missing or
/// unreadable.
pub fn resolve_reconnect_settings() -> (bool, u32) {
    let mtime = get_config_path()
        .ok()
        .map(|dir| dir.join("config.json"))
        .and_then(|path| fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok());

    if let Some(cached) = RECONNECT_SETTINGS_CACHE
        .read()
        .ok()
        .and_then(|guard| reconnect_settings_cache_hit(*guard, mtime))
    {
        return cached;
    }

    let settings = load_config_file()
        .map(|config| (config.reconnect_enabled, config.reconnect_max_attempts))
        .unwrap_or_else(|_| (default_reconnect_enabled(), default_reconnect_max_attempts()));

    if let Ok(mut cache) = RECONNECT_SETTINGS_CACHE.write() {
        *cache = Some((mtime, settings.0, settings.1));
    }
    settings
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
    let mut config = config;
    // The storage mode changes only through the secret store commands, which
    // also move the keys; a stale value from the UI must not overwrite it.
    config.secret_storage_mode = load_config_file()?.secret_storage_mode;
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
        assert_eq!(
            config.monitor_visible_metrics,
            ["cpu", "memory", "network", "ip", "latency", "disk"]
        );
        assert_eq!(config.sftp_transfer_parallelism, 4);
        assert!(config.reconnect_enabled);
        assert_eq!(config.reconnect_max_attempts, 5);
    }

    #[test]
    fn explicit_sftp_transfer_parallelism_round_trips() {
        let config: AppConfig =
            serde_json::from_str(r#"{"theme":"default","sftp_transfer_parallelism":6}"#).unwrap();

        assert_eq!(config.sftp_transfer_parallelism, 6);
    }

    #[test]
    fn reconnect_enabled_round_trips() {
        let disabled: AppConfig =
            serde_json::from_str(r#"{"theme":"default","reconnect_enabled":false}"#).unwrap();
        assert!(!disabled.reconnect_enabled);

        let enabled: AppConfig =
            serde_json::from_str(r#"{"theme":"default","reconnect_enabled":true}"#).unwrap();
        assert!(enabled.reconnect_enabled);

        let serialized = serde_json::to_string(&disabled).unwrap();
        let reparsed: AppConfig = serde_json::from_str(&serialized).unwrap();
        assert!(!reparsed.reconnect_enabled);
    }

    #[test]
    fn reconnect_max_attempts_round_trips() {
        let custom: AppConfig =
            serde_json::from_str(r#"{"theme":"default","reconnect_max_attempts":12}"#).unwrap();
        assert_eq!(custom.reconnect_max_attempts, 12);

        let legacy: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();
        assert_eq!(legacy.reconnect_max_attempts, 5);
    }

    #[test]
    fn legacy_config_gets_default_keymap() {
        let config: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();

        assert!(config.keymap.bindings.is_empty());
    }

    #[test]
    fn keymap_config_round_trips() {
        let config: AppConfig = serde_json::from_str(
            r#"{"theme":"default","keymap":{"bindings":{"terminal.find":["mod+j"],"terminal.clear":null}}}"#,
        )
        .unwrap();

        assert_eq!(
            config.keymap.bindings.get("terminal.find"),
            Some(&Some(vec!["mod+j".to_string()]))
        );
        assert_eq!(config.keymap.bindings.get("terminal.clear"), Some(&None));

        let serialized = serde_json::to_string(&config).unwrap();
        let reparsed: AppConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            reparsed.keymap.bindings.get("terminal.find"),
            config.keymap.bindings.get("terminal.find")
        );
    }

    #[test]
    fn keymap_config_ignores_removed_preset_field() {
        let config: AppConfig =
            serde_json::from_str(r#"{"theme":"default","keymap":{"preset":"vscode","bindings":{}}}"#)
                .unwrap();

        assert!(config.keymap.bindings.is_empty());
        assert!(!serde_json::to_string(&config).unwrap().contains("preset"));
    }

    #[test]
    fn keymap_config_accepts_legacy_single_chord_values() {
        let config: AppConfig = serde_json::from_str(
            r#"{"theme":"default","keymap":{"bindings":{"terminal.find":"mod+j","editor.save":null}}}"#,
        )
        .unwrap();

        assert_eq!(
            config.keymap.bindings.get("terminal.find"),
            Some(&Some(vec!["mod+j".to_string()]))
        );
        assert_eq!(config.keymap.bindings.get("editor.save"), Some(&None));
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
