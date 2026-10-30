pub(crate) mod atomic;
mod defaults;
mod deserialize;
pub mod keymap;
pub mod keyword_highlight;
pub mod local_shells;
pub mod panes;
pub(crate) mod paths;
pub mod reconnect;
pub mod tab_agent_indicator;
pub mod tab_icons;
mod update_channel;
pub mod window_background;

use atomic::atomic_write;
use keymap::KeymapConfig;
use paths::{ensure_config_dir, get_config_path};

use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub theme: String,
    /// Switch between `theme_light` and `theme_dark` with the system's
    /// appearance instead of showing `theme`.
    #[serde(default)]
    pub theme_follow_system: bool,
    #[serde(default = "defaults::default_theme_light")]
    pub theme_light: String,
    #[serde(default = "defaults::default_theme_dark")]
    pub theme_dark: String,
    /// Theme ids starred in the theme library.
    #[serde(default)]
    pub favorite_themes: Vec<String>,
    #[serde(default = "defaults::default_language")]
    pub language: String,
    #[serde(default = "defaults::default_font_family")]
    pub font_family: String,
    #[serde(default = "defaults::default_font_size")]
    pub font_size: u16,
    #[serde(
        default = "defaults::default_ui_scale_percent",
        deserialize_with = "deserialize::deserialize_ui_scale_percent"
    )]
    pub ui_scale_percent: u16,
    #[serde(default = "defaults::default_cursor_style")]
    pub cursor_style: String,
    #[serde(default = "defaults::default_terminal_shell")]
    pub terminal_shell: String,
    #[serde(default)]
    pub terminal_shell_custom_path: String,
    #[serde(default)]
    pub terminal_shell_custom_args: String,
    /// The shell the user added that is the default, when it is one: the
    /// default's path and arguments above are kept equal to its own.
    #[serde(default)]
    pub terminal_shell_id: String,
    /// The WSL distribution the default shell starts when it is `wsl`;
    /// empty starts WSL's own default.
    #[serde(default)]
    pub terminal_shell_wsl_distro: String,
    /// Shells the user added for local tabs, kept per device.
    #[serde(default)]
    pub local_shells: Vec<local_shells::LocalShell>,
    /// Detected shells the user hid from the new tab list, by key (`pwsh`, `wsl:Ubuntu`,
    /// or the path of one found on PATH).
    #[serde(default)]
    pub hidden_shells: Vec<String>,
    #[serde(default = "defaults::default_secret_vault_enabled")]
    pub secret_vault_enabled: bool,
    #[serde(default = "defaults::default_secret_storage_mode")]
    pub secret_storage_mode: String,
    #[serde(default)]
    pub prompt_unlock_vault_on_startup: bool,
    #[serde(default = "defaults::default_scrollback_lines")]
    pub scrollback_lines: u32,
    #[serde(default = "defaults::default_terminal_renderer")]
    pub terminal_renderer: String,
    /// Seconds a terminal stays out of sight before its renderer is released; 0 keeps it.
    #[serde(default = "defaults::default_hidden_renderer_release_secs")]
    pub hidden_renderer_release_secs: u32,
    #[serde(default = "defaults::default_terminal_padding_left_px")]
    pub terminal_padding_left_px: u16,
    #[serde(default = "defaults::default_terminal_padding_right_px")]
    pub terminal_padding_right_px: u16,
    #[serde(default = "defaults::default_terminal_padding_bottom_px")]
    pub terminal_padding_bottom_px: u16,
    #[serde(default = "defaults::default_startup_session_restore_mode")]
    pub startup_session_restore_mode: String,
    /// Whether a restart shows each terminal tab's content from before.
    #[serde(default = "defaults::default_true")]
    pub restore_terminal_content: bool,
    #[serde(default = "defaults::default_show_jump_host_connection_info")]
    pub show_jump_host_connection_info: bool,
    #[serde(default)]
    pub sftp_paste_upload_enabled: bool,
    /// Where the SFTP drawer opens: `home`, `last` or `terminal`; a
    /// connection can choose otherwise.
    #[serde(default = "defaults::default_sftp_start_directory")]
    pub sftp_start_directory: String,
    #[serde(default = "defaults::default_monitor_refresh_interval_secs")]
    pub monitor_refresh_interval_secs: u16,
    #[serde(default = "defaults::default_monitor_visible_metrics")]
    pub monitor_visible_metrics: Vec<String>,
    #[serde(default = "update_channel::default_update_channel")]
    pub update_channel: String,
    #[serde(default = "defaults::default_auto_download_updates")]
    pub auto_download_updates: bool,
    #[serde(default = "defaults::default_update_check_frequency")]
    pub update_check_frequency: String,
    #[serde(
        default = "defaults::default_tab_width_mode",
        deserialize_with = "deserialize::deserialize_tab_width_mode"
    )]
    pub tab_width_mode: String,
    #[serde(
        default = "defaults::default_tab_standard_width",
        deserialize_with = "deserialize::deserialize_tab_standard_width"
    )]
    pub tab_standard_width: u16,
    /// How the connections panel lists profiles: "default" by group, or
    /// "recent" in one list with the one used last first.
    #[serde(
        default = "defaults::default_profile_sort",
        deserialize_with = "deserialize::deserialize_profile_sort"
    )]
    pub profile_sort: String,
    /// Show a tab's full title and connection in a tooltip on hover.
    #[serde(default)]
    pub tab_hover_tooltip: bool,
    #[serde(default)]
    pub terminal_log_enabled: bool,
    #[serde(default)]
    pub terminal_log_directory: String,
    #[serde(default = "defaults::default_terminal_log_format")]
    pub terminal_log_format: String,
    #[serde(default = "defaults::default_terminal_log_name_template")]
    pub terminal_log_name_template: String,
    #[serde(default = "defaults::default_terminal_log_max_file_size_mb")]
    pub terminal_log_max_file_size_mb: u32,
    #[serde(default)]
    pub terminal_log_compress: bool,
    /// Start each line of the plain-text log with when it appeared.
    #[serde(default = "defaults::default_true")]
    pub terminal_log_plain_timestamps: bool,
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
    #[serde(default = "defaults::default_sftp_transfer_parallelism")]
    pub sftp_transfer_parallelism: u16,
    /// SFTP upload bandwidth cap in KiB/s shared by all transfers; 0 = none.
    #[serde(default)]
    pub sftp_upload_limit_kib: u32,
    /// SFTP download bandwidth cap in KiB/s shared by all transfers; 0 = none.
    #[serde(default)]
    pub sftp_download_limit_kib: u32,
    /// Automatically re-establish dropped SSH sessions with capped backoff.
    #[serde(default = "defaults::default_reconnect_enabled")]
    pub reconnect_enabled: bool,
    /// How many automatic reconnect attempts to make before giving up.
    #[serde(default = "defaults::default_reconnect_max_attempts")]
    pub reconnect_max_attempts: u32,
    /// Passively watch interactive shell sessions for a ZMODEM (`rz`/`sz`)
    /// invite and auto-start the transfer, mirroring SecureCRT/Xshell.
    #[serde(default = "defaults::default_zmodem_auto_detect_enabled")]
    pub zmodem_auto_detect_enabled: bool,
    /// Where auto-detected ZMODEM downloads are saved; empty resolves to the
    /// platform Downloads directory at time of use.
    #[serde(default)]
    pub zmodem_download_directory: String,
    /// Extra sudo password prompt patterns (JavaScript regular expressions)
    /// matched by the frontend on top of the built-in ones.
    #[serde(default)]
    pub sudo_prompt_patterns: Vec<String>,
    /// Whether the terminal shows a mark per character typed at a password
    /// prompt, as sudo's pwfeedback does; off, it shows only a key.
    #[serde(default = "defaults::default_password_feedback")]
    pub password_feedback: bool,
    /// Send Option+key as Meta (ESC prefix) in the terminal instead of typing
    /// the macOS alternate character. Only read on macOS.
    #[serde(default)]
    pub mac_option_is_meta: bool,
    /// Ask before pasting text with line breaks into a shell that would run
    /// each line as it arrives (one without bracketed paste mode).
    #[serde(default = "defaults::default_confirm_multiline_paste")]
    pub confirm_multiline_paste: bool,
    /// Copy the terminal selection to the clipboard when the mouse button is
    /// released, as PuTTY and Xshell do.
    #[serde(default)]
    pub copy_on_select: bool,
    /// Right-clicking the terminal pastes instead of opening the context menu
    /// (Shift+right-click still opens it). On by default on Windows, where
    /// PuTTY, Xshell and Windows Terminal paste on right click.
    #[serde(default = "defaults::default_right_click_paste")]
    pub right_click_paste: bool,
    /// Let programs in the terminal copy text to the clipboard with OSC 52,
    /// which is how vim, neovim and tmux copy over SSH. Reading the clipboard
    /// that way is never allowed.
    #[serde(default = "defaults::default_osc52_clipboard_write")]
    pub osc52_clipboard_write: bool,
    /// Local shells mark their prompts and commands (OSC 133), so the
    /// terminal can jump between commands, copy a command's output and show
    /// commands on the scrollbar.
    #[serde(default = "defaults::default_command_marks")]
    pub command_marks: bool,
    /// macOS and Linux: local bash, zsh and fish start through tTerm's shell
    /// integration, so they mark their commands as well. Off by default, as
    /// it changes how the shell starts; Windows shells always have it.
    #[serde(default)]
    pub local_shell_integration: bool,
    /// Terminal line height as a multiple of the font's cell height.
    #[serde(
        default = "defaults::default_terminal_line_height",
        deserialize_with = "deserialize::deserialize_terminal_line_height"
    )]
    pub terminal_line_height: f64,
    /// Extra pixels between terminal characters; may be negative.
    #[serde(
        default,
        deserialize_with = "deserialize::deserialize_terminal_letter_spacing"
    )]
    pub terminal_letter_spacing: i8,
    #[serde(default)]
    pub keymap: KeymapConfig,
    /// What WebKit does with the page while the window is hidden (macOS 14+):
    /// "throttle", "disabled" or "suspend". Read once when the window is
    /// created, so a change applies after restart.
    #[serde(default = "defaults::default_background_throttling")]
    pub background_throttling: String,
    /// Blur whatever is behind the window (macOS, Windows) and tint it with
    /// the theme background instead of painting that background solid.
    #[serde(default)]
    pub window_blur: bool,
    /// Blur radius in points (macOS; Windows has no radius).
    #[serde(
        default = "defaults::default_window_blur_radius",
        deserialize_with = "deserialize::deserialize_window_blur_radius"
    )]
    pub window_blur_radius: u8,
    /// Opacity of the theme tint over the blur, in percent.
    #[serde(
        default = "defaults::default_window_opacity_percent",
        deserialize_with = "deserialize::deserialize_window_opacity_percent"
    )]
    pub window_opacity_percent: u8,
    /// Windows backdrop: "acrylic" blurs what is behind the window, "mica"
    /// tints it with the desktop wallpaper (Windows 11).
    #[serde(default = "defaults::default_window_blur_material")]
    pub window_blur_material: String,
    /// How the window lays out its surfaces: "classic", or "islands", which
    /// floats each pane as a rounded card on the title bar's color.
    #[serde(default = "defaults::default_ui_style")]
    pub ui_style: String,
    /// What closing the main window does: "ask", "tray" (keep running in the
    /// background behind a tray icon) or "quit".
    #[serde(default = "defaults::default_close_behavior")]
    pub close_behavior: String,
    /// Announce terminal events (finished commands, notifications programs
    /// ask for, bells) while the user looks elsewhere.
    #[serde(default = "defaults::default_true")]
    pub notifications_enabled: bool,
    /// Announce commands that ran at least `notify_command_min_secs`; needs
    /// shell integration to know when a command starts and ends.
    #[serde(default = "defaults::default_true")]
    pub notify_command_finished: bool,
    #[serde(
        default = "defaults::default_notify_command_min_secs",
        deserialize_with = "deserialize::deserialize_notify_command_min_secs"
    )]
    pub notify_command_min_secs: u16,
    /// Announce notifications programs send (OSC 9, OSC 777, OSC 99).
    #[serde(default = "defaults::default_true")]
    pub notify_terminal_requests: bool,
    /// Announce an AI agent waiting for the user (tTerm's agent hooks).
    #[serde(default = "defaults::default_true")]
    pub notify_agent_waiting: bool,
    /// Announce an AI agent finishing its turn, or stopping on an error.
    #[serde(default = "defaults::default_true")]
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
        default = "defaults::default_toast_max_visible",
        deserialize_with = "deserialize::deserialize_toast_max_visible"
    )]
    pub toast_max_visible: u8,
    /// System notifications make a sound.
    #[serde(default = "defaults::default_true")]
    pub notify_sound: bool,
    /// Which sound: "system", one tTerm plays itself ("chime", "ding", "pop",
    /// "rise"), or "custom" for `notify_sound_file`.
    #[serde(default = "defaults::default_notify_sound_name")]
    pub notify_sound_name: String,
    /// The name of the file the "custom" sound was copied from; tTerm plays
    /// its own copy (`notification::sound`).
    #[serde(default)]
    pub notify_sound_file: String,
    /// What the terminal bell does: "visual", "sound" or "none".
    #[serde(default = "defaults::default_bell_style")]
    pub bell_style: String,
    /// How local terminal tabs show their shell's icon.
    #[serde(default)]
    pub tab_icons: tab_icons::TabIconConfig,
    /// How a tab shows an AI agent at work or waiting for the user.
    #[serde(default)]
    pub tab_agent_indicator: tab_agent_indicator::TabAgentIndicatorConfig,
    /// Words and patterns colored in terminal output, in named sets.
    #[serde(default)]
    pub keyword_highlight: keyword_highlight::KeywordHighlightConfig,
    /// How split panes look, and the saved split layouts.
    #[serde(default)]
    pub panes: panes::PaneConfig,
    /// Proxy server that hosts left on the default connect through; none when empty.
    #[serde(default)]
    pub default_proxy_id: String,
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
    let mut config: AppConfig =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;
    local_shells::migrate(&mut config);
    local_shells::migrate_hidden_wsl(&mut config, crate::terminal::pty::wsl_distro_names);
    Ok(config)
}

pub fn save_config_file(config: &AppConfig) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let config_file = config_dir.join("config.json");
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    atomic_write(&config_file, content)
}

/// Whether shells mark their commands (OSC 133); on when the config file is
/// missing or unreadable.
pub fn command_marks_enabled() -> bool {
    load_config_file()
        .map(|config| config.command_marks)
        .unwrap_or_else(|_| defaults::default_command_marks())
}

/// Whether tabs show icons, which for an SSH tab means asking the host what
/// it runs; on when the config file is missing or unreadable.
pub fn tab_icons_enabled() -> bool {
    load_config_file()
        .map(|config| config.tab_icons.enabled)
        .unwrap_or(true)
}

/// Whether local shells on macOS and Linux start through tTerm's shell
/// integration; off when the config file is missing or unreadable.
#[cfg(unix)]
pub fn local_shell_integration_enabled() -> bool {
    load_config_file().is_ok_and(|config| config.local_shell_integration)
}

#[tauri::command]
pub async fn load_config() -> Result<AppConfig, String> {
    crate::core::blocking::run_blocking(load_config_file).await
}

#[tauri::command]
pub async fn save_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    crate::core::blocking::run_blocking(move || {
        use tauri::Manager;
        let log_state = app.state::<crate::session_log::SessionLogState>();
        let mut config = config;
        // The storage mode changes only through the secret store commands, which
        // also move the keys; a stale value from the UI must not overwrite it.
        config.secret_storage_mode = load_config_file()?.secret_storage_mode;
        log_state.validate_config(&config)?;
        save_config_file(&config)?;
        crate::sftp::internal::api::apply_bandwidth_limits(&config);
        crate::background::apply_config(&app, &config);
        crate::system_integration::apply_config(&app, &config);
        log_state.apply_config(&app, &config)
    })
    .await
}

#[cfg(test)]
mod tests;
