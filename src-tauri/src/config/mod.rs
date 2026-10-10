mod atomic;
mod paths;
pub mod window_background;

pub use atomic::{atomic_write, atomic_write_private};
pub use paths::{ensure_config_dir, get_config_path, init_config_dir, legacy_config_path};
pub use window_background::load_window_background;

use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::sync::RwLock;
use std::time::SystemTime;
use sys_locale::get_locale;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub theme: String,
    /// Switch between `theme_light` and `theme_dark` with the system's
    /// appearance instead of showing `theme`.
    #[serde(default)]
    pub theme_follow_system: bool,
    #[serde(default = "default_theme_light")]
    pub theme_light: String,
    #[serde(default = "default_theme_dark")]
    pub theme_dark: String,
    /// Theme ids starred in the theme library.
    #[serde(default)]
    pub favorite_themes: Vec<String>,
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
    /// Seconds a terminal stays out of sight before its renderer is released; 0 keeps it.
    #[serde(default = "default_hidden_renderer_release_secs")]
    pub hidden_renderer_release_secs: u32,
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
    /// Also log what is typed, including passwords at prompts that do not echo.
    #[serde(default)]
    pub terminal_log_record_input: bool,
    /// Remove logs last written more than this many days ago; 0 keeps them.
    #[serde(default)]
    pub terminal_log_retention_days: u32,
    /// Remove the oldest logs while all of them take more than this many
    /// MiB; 0 is no limit.
    #[serde(default)]
    pub terminal_log_max_total_mb: u32,
    #[serde(default = "default_sftp_transfer_parallelism")]
    pub sftp_transfer_parallelism: u16,
    /// SFTP upload bandwidth cap in KiB/s shared by all transfers; 0 = none.
    #[serde(default)]
    pub sftp_upload_limit_kib: u32,
    /// SFTP download bandwidth cap in KiB/s shared by all transfers; 0 = none.
    #[serde(default)]
    pub sftp_download_limit_kib: u32,
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
    /// Extra sudo password prompt patterns (JavaScript regular expressions)
    /// matched by the frontend on top of the built-in ones.
    #[serde(default)]
    pub sudo_prompt_patterns: Vec<String>,
    /// Send Option+key as Meta (ESC prefix) in the terminal instead of typing
    /// the macOS alternate character. Only read on macOS.
    #[serde(default)]
    pub mac_option_is_meta: bool,
    /// Ask before pasting text with line breaks into a shell that would run
    /// each line as it arrives (one without bracketed paste mode).
    #[serde(default = "default_confirm_multiline_paste")]
    pub confirm_multiline_paste: bool,
    /// Copy the terminal selection to the clipboard when the mouse button is
    /// released, as PuTTY and Xshell do.
    #[serde(default)]
    pub copy_on_select: bool,
    /// Right-clicking the terminal pastes instead of opening the context menu
    /// (Shift+right-click still opens it).
    #[serde(default)]
    pub right_click_paste: bool,
    /// Local shells mark their prompts and commands (OSC 133), so the
    /// terminal can jump between commands, copy a command's output and show
    /// commands on the scrollbar.
    #[serde(default = "default_command_marks")]
    pub command_marks: bool,
    /// macOS and Linux: local bash, zsh and fish start through tTerm's shell
    /// integration, so they mark their commands as well. Off by default, as
    /// it changes how the shell starts; Windows shells always have it.
    #[serde(default)]
    pub local_shell_integration: bool,
    /// Terminal line height as a multiple of the font's cell height.
    #[serde(
        default = "default_terminal_line_height",
        deserialize_with = "deserialize_terminal_line_height"
    )]
    pub terminal_line_height: f64,
    /// Extra pixels between terminal characters; may be negative.
    #[serde(default, deserialize_with = "deserialize_terminal_letter_spacing")]
    pub terminal_letter_spacing: i8,
    #[serde(default = "default_keymap")]
    pub keymap: KeymapConfig,
    /// What WebKit does with the page while the window is hidden (macOS 14+):
    /// "throttle", "disabled" or "suspend". Read once when the window is
    /// created, so a change applies after restart.
    #[serde(default = "default_background_throttling")]
    pub background_throttling: String,
    /// Blur whatever is behind the window (macOS, Windows) and tint it with
    /// the theme background instead of painting that background solid.
    #[serde(default)]
    pub window_blur: bool,
    /// Blur radius in points (macOS; Windows has no radius).
    #[serde(
        default = "default_window_blur_radius",
        deserialize_with = "deserialize_window_blur_radius"
    )]
    pub window_blur_radius: u8,
    /// Opacity of the theme tint over the blur, in percent.
    #[serde(
        default = "default_window_opacity_percent",
        deserialize_with = "deserialize_window_opacity_percent"
    )]
    pub window_opacity_percent: u8,
    /// Windows backdrop: "acrylic" blurs what is behind the window, "mica"
    /// tints it with the desktop wallpaper (Windows 11).
    #[serde(default = "default_window_blur_material")]
    pub window_blur_material: String,
    /// What closing the main window does: "ask", "tray" (keep running in the
    /// background behind a tray icon) or "quit".
    #[serde(default = "default_close_behavior")]
    pub close_behavior: String,
    /// Announce terminal events (finished commands, notifications programs
    /// ask for, bells) while the user looks elsewhere.
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,
    /// Announce commands that ran at least `notify_command_min_secs`; needs
    /// shell integration to know when a command starts and ends.
    #[serde(default = "default_true")]
    pub notify_command_finished: bool,
    #[serde(
        default = "default_notify_command_min_secs",
        deserialize_with = "deserialize_notify_command_min_secs"
    )]
    pub notify_command_min_secs: u16,
    /// Announce notifications programs send (OSC 9, OSC 777, OSC 99).
    #[serde(default = "default_true")]
    pub notify_terminal_requests: bool,
    /// Announce an AI agent waiting for the user (tTerm's agent hooks).
    #[serde(default = "default_true")]
    pub notify_agent_waiting: bool,
    /// Announce an AI agent finishing its turn, or stopping on an error.
    #[serde(default = "default_true")]
    pub notify_agent_done: bool,
    /// AI agents' config directories chosen by hand, by agent (`claudeCode`,
    /// `codex`, `openCode`, `pi`); others are found from the agent's variable
    /// or its default.
    #[serde(default)]
    pub agent_config_dirs: std::collections::BTreeMap<String, String>,
    /// Announce a bell in a terminal the user is not looking at.
    #[serde(default)]
    pub bell_notify: bool,
    /// In-app toasts stacked at once; a new one closes the oldest past this.
    #[serde(
        default = "default_toast_max_visible",
        deserialize_with = "deserialize_toast_max_visible"
    )]
    pub toast_max_visible: u8,
    /// System notifications play the system sound.
    #[serde(default = "default_true")]
    pub notify_sound: bool,
    /// What the terminal bell does: "visual", "sound" or "none".
    #[serde(default = "default_bell_style")]
    pub bell_style: String,
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

fn default_window_blur_radius() -> u8 {
    20
}

fn deserialize_window_blur_radius<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(1.0, 60.0) as u8)
}

fn default_window_blur_material() -> String {
    "acrylic".to_string()
}

fn default_window_opacity_percent() -> u8 {
    70
}

fn deserialize_window_opacity_percent<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(30.0, 95.0) as u8)
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
    "webgl".to_string()
}

fn default_hidden_renderer_release_secs() -> u32 {
    10
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

fn default_confirm_multiline_paste() -> bool {
    true
}

fn default_command_marks() -> bool {
    true
}

fn default_terminal_line_height() -> f64 {
    1.0
}

fn deserialize_terminal_line_height<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(((value * 100.0).round() / 100.0).clamp(1.0, 2.0))
}

fn deserialize_terminal_letter_spacing<'de, D>(deserializer: D) -> Result<i8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(-5.0, 10.0) as i8)
}

fn default_reconnect_max_attempts() -> u32 {
    5
}

fn default_background_throttling() -> String {
    "throttle".to_string()
}

fn default_close_behavior() -> String {
    "ask".to_string()
}

fn default_true() -> bool {
    true
}

fn default_notify_command_min_secs() -> u16 {
    10
}

fn deserialize_notify_command_min_secs<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(1.0, 3600.0) as u16)
}

fn default_toast_max_visible() -> u8 {
    3
}

fn deserialize_toast_max_visible<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(value.round().clamp(1.0, 5.0) as u8)
}

fn default_bell_style() -> String {
    "none".to_string()
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
            theme_follow_system: false,
            theme_light: default_theme_light(),
            theme_dark: default_theme_dark(),
            favorite_themes: Vec::new(),
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
            hidden_renderer_release_secs: default_hidden_renderer_release_secs(),
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
            tab_width_mode: default_tab_width_mode(),
            tab_standard_width: default_tab_standard_width(),
            terminal_log_enabled: false,
            terminal_log_directory: String::new(),
            terminal_log_format: default_terminal_log_format(),
            terminal_log_name_template: default_terminal_log_name_template(),
            terminal_log_max_file_size_mb: default_terminal_log_max_file_size_mb(),
            terminal_log_compress: false,
            terminal_log_record_input: false,
            terminal_log_retention_days: 0,
            terminal_log_max_total_mb: 0,
            sftp_transfer_parallelism: default_sftp_transfer_parallelism(),
            sftp_upload_limit_kib: 0,
            sftp_download_limit_kib: 0,
            reconnect_enabled: default_reconnect_enabled(),
            reconnect_max_attempts: default_reconnect_max_attempts(),
            zmodem_auto_detect_enabled: default_zmodem_auto_detect_enabled(),
            zmodem_download_directory: String::new(),
            sudo_prompt_patterns: Vec::new(),
            mac_option_is_meta: false,
            confirm_multiline_paste: default_confirm_multiline_paste(),
            copy_on_select: false,
            right_click_paste: false,
            command_marks: default_command_marks(),
            local_shell_integration: false,
            terminal_line_height: default_terminal_line_height(),
            terminal_letter_spacing: 0,
            keymap: default_keymap(),
            background_throttling: default_background_throttling(),
            window_blur: false,
            window_blur_radius: default_window_blur_radius(),
            window_opacity_percent: default_window_opacity_percent(),
            window_blur_material: default_window_blur_material(),
            close_behavior: default_close_behavior(),
            notifications_enabled: true,
            notify_command_finished: true,
            notify_command_min_secs: default_notify_command_min_secs(),
            notify_terminal_requests: true,
            notify_agent_waiting: true,
            notify_agent_done: true,
            agent_config_dirs: Default::default(),
            bell_notify: false,
            toast_max_visible: default_toast_max_visible(),
            notify_sound: true,
            bell_style: default_bell_style(),
        }
    }
}

fn default_theme_light() -> String {
    "light".to_string()
}

fn default_theme_dark() -> String {
    "default".to_string()
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

/// Whether shells mark their commands (OSC 133); on when the config file is
/// missing or unreadable.
pub fn command_marks_enabled() -> bool {
    load_config_file()
        .map(|config| config.command_marks)
        .unwrap_or_else(|_| default_command_marks())
}

/// Whether local shells on macOS and Linux start through tTerm's shell
/// integration; off when the config file is missing or unreadable.
pub fn local_shell_integration_enabled() -> bool {
    load_config_file().is_ok_and(|config| config.local_shell_integration)
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
        .unwrap_or_else(|_| {
            (
                default_reconnect_enabled(),
                default_reconnect_max_attempts(),
            )
        });

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
    crate::sftp::internal::api::apply_bandwidth_limits(&config);
    crate::background::apply_config(&app, &config);
    log_state.apply_config(&app, &config)
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn legacy_config_shows_its_one_theme() {
        let config: AppConfig = serde_json::from_str(r#"{"theme":"catalog:Nord"}"#).unwrap();

        assert_eq!(config.theme, "catalog:Nord");
        assert!(!config.theme_follow_system);
        assert_eq!(config.theme_light, "light");
        assert_eq!(config.theme_dark, "default");
        assert!(config.favorite_themes.is_empty());
    }

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
    fn sftp_bandwidth_limits_default_to_unlimited_and_round_trip() {
        let legacy: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();
        assert_eq!(legacy.sftp_upload_limit_kib, 0);
        assert_eq!(legacy.sftp_download_limit_kib, 0);

        let config: AppConfig = serde_json::from_str(
            r#"{"theme":"default","sftp_upload_limit_kib":512,"sftp_download_limit_kib":2048}"#,
        )
        .unwrap();
        assert_eq!(config.sftp_upload_limit_kib, 512);
        assert_eq!(config.sftp_download_limit_kib, 2048);
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
        let config: AppConfig = serde_json::from_str(
            r#"{"theme":"default","keymap":{"preset":"vscode","bindings":{}}}"#,
        )
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
    fn terminal_line_height_and_letter_spacing_are_clamped() {
        let config: AppConfig = serde_json::from_str(
            r#"{"theme":"default","terminal_line_height":1.234,"terminal_letter_spacing":-1.6}"#,
        )
        .unwrap();
        assert_eq!(config.terminal_line_height, 1.23);
        assert_eq!(config.terminal_letter_spacing, -2);

        let config: AppConfig = serde_json::from_str(
            r#"{"theme":"default","terminal_line_height":0.5,"terminal_letter_spacing":40}"#,
        )
        .unwrap();
        assert_eq!(config.terminal_line_height, 1.0);
        assert_eq!(config.terminal_letter_spacing, 10);
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

    #[test]
    fn window_blur_config_is_clamped() {
        let parse = |field: &str, value: &str| -> AppConfig {
            serde_json::from_str(&format!(r#"{{"theme":"default","{field}":{value}}}"#)).unwrap()
        };

        assert_eq!(
            parse("window_opacity_percent", "62.4").window_opacity_percent,
            62
        );
        assert_eq!(
            parse("window_opacity_percent", "10").window_opacity_percent,
            30
        );
        assert_eq!(
            parse("window_opacity_percent", "100").window_opacity_percent,
            95
        );
        assert_eq!(parse("window_blur_radius", "0").window_blur_radius, 1);
        assert_eq!(parse("window_blur_radius", "200").window_blur_radius, 60);
        let missing: AppConfig = serde_json::from_str(r#"{"theme":"default"}"#).unwrap();
        assert!(!missing.window_blur);
        assert_eq!(missing.window_blur_radius, 20);
        assert_eq!(missing.window_opacity_percent, 70);
        assert_eq!(missing.window_blur_material, "acrylic");
    }
}
