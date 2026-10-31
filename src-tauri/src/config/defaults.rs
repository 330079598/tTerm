//! What a first launch starts from, and what a config file missing a setting
//! falls back to.

use super::keymap::KeymapConfig;
use super::update_channel::default_update_channel;
use super::{keyword_highlight, panes, tab_agent_indicator, tab_icons, AppConfig};
use sys_locale::get_locale;

fn normalize_language(locale: &str) -> String {
    let normalized_locale = locale.replace('_', "-").to_ascii_lowercase();

    if normalized_locale.starts_with("zh") {
        return "zh".to_string();
    }

    "en".to_string()
}

pub(super) fn default_language() -> String {
    get_locale()
        .map(|locale| normalize_language(&locale))
        .unwrap_or_else(|| "en".to_string())
}

/// A Nerd Font first, so prompts such as starship and powerlevel10k draw
/// their icons, then the system's monospace font. The frontend's
/// `defaultTerminalFontFamily` holds the same chains, which a test checks.
pub(super) fn default_font_family() -> String {
    #[cfg(target_os = "macos")]
    return "\"JetBrains Mono Nerd Font\", \"JetBrainsMono Nerd Font\", \"JetBrains Mono\", Menlo, Monaco, monospace".to_string();
    #[cfg(target_os = "windows")]
    return "\"JetBrains Mono Nerd Font\", \"JetBrainsMono Nerd Font\", \"JetBrains Mono\", \"Cascadia Code\", Consolas, monospace".to_string();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return "\"JetBrains Mono Nerd Font\", \"JetBrainsMono Nerd Font\", \"JetBrains Mono\", \"DejaVu Sans Mono\", monospace".to_string();
}

pub(super) fn default_font_size() -> u16 {
    14
}

pub(super) fn default_ui_scale_percent() -> u16 {
    100
}

pub(super) fn default_window_blur_radius() -> u8 {
    20
}

pub(super) fn default_window_blur_material() -> String {
    "acrylic".to_string()
}

pub(super) fn default_window_opacity_percent() -> u8 {
    70
}

pub(super) fn default_cursor_style() -> String {
    "block".to_string()
}

pub(super) fn default_terminal_shell() -> String {
    "auto".to_string()
}

/// Older versions' default. It tells the one-time password migration where
/// passwords were kept; afterwards `hybrid` means the same as `system`.
pub(super) fn default_secret_storage_mode() -> String {
    if cfg!(target_os = "windows") {
        "system".to_string()
    } else {
        "hybrid".to_string()
    }
}

pub(super) fn default_secret_vault_enabled() -> bool {
    true
}

pub(super) fn default_scrollback_lines() -> u32 {
    10000
}

pub(super) fn default_terminal_renderer() -> String {
    "webgl".to_string()
}

pub(super) fn default_hidden_renderer_release_secs() -> u32 {
    10
}

pub(super) fn default_terminal_padding_left_px() -> u16 {
    6
}

pub(super) fn default_terminal_padding_right_px() -> u16 {
    6
}

pub(super) fn default_terminal_padding_bottom_px() -> u16 {
    4
}

pub(super) fn default_startup_session_restore_mode() -> String {
    "active".to_string()
}

pub(super) fn default_show_jump_host_connection_info() -> bool {
    true
}

pub(super) fn default_password_feedback() -> bool {
    true
}

pub(super) fn default_predictive_echo() -> String {
    "auto".to_string()
}

pub(super) fn default_predictive_echo_threshold_ms() -> u16 {
    30
}

pub(super) fn default_monitor_refresh_interval_secs() -> u16 {
    5
}

pub(super) fn default_monitor_visible_metrics() -> Vec<String> {
    ["cpu", "memory", "network", "ip", "latency", "disk"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

pub(super) fn default_auto_download_updates() -> bool {
    true
}

pub(super) fn default_update_check_frequency() -> String {
    "daily".to_string()
}

pub(super) fn default_tab_width_mode() -> String {
    "adaptive".to_string()
}

pub(super) fn default_tab_standard_width() -> u16 {
    120
}

pub(super) fn default_terminal_log_format() -> String {
    "both".to_string()
}

pub(super) fn default_terminal_log_name_template() -> String {
    "{year}-{month}-{day}/{profile}-{host}-{time}-{sessionId}".to_string()
}

pub(super) fn default_terminal_log_max_file_size_mb() -> u32 {
    50
}

pub(super) fn default_sftp_start_directory() -> String {
    "last".to_string()
}

pub(super) fn default_sftp_external_upload() -> String {
    "ask".to_string()
}

pub(super) fn default_sftp_transfer_parallelism() -> u16 {
    4
}

pub(super) fn default_reconnect_enabled() -> bool {
    true
}

pub(super) fn default_right_click_paste() -> bool {
    cfg!(target_os = "windows")
}

pub(super) fn default_confirm_multiline_paste() -> bool {
    true
}

pub(super) fn default_osc52_clipboard_write() -> bool {
    true
}

pub(super) fn default_command_marks() -> bool {
    true
}

pub(super) fn default_local_unix_shell_integration() -> bool {
    cfg!(target_os = "macos")
}

pub(super) fn default_ssh_shell_integration() -> bool {
    true
}

pub(super) fn default_terminal_line_height() -> f64 {
    1.0
}

pub(super) fn default_reconnect_max_attempts() -> u32 {
    5
}

pub(super) fn default_background_throttling() -> String {
    "throttle".to_string()
}

/// The layout for a config file saved before the setting existed. Such a
/// user has only ever seen the classic layout, so an update keeps it; a first
/// launch starts in islands instead (see `AppConfig::default`).
pub(super) fn default_ui_style() -> String {
    "classic".to_string()
}

pub(super) fn default_close_behavior() -> String {
    "ask".to_string()
}

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_notify_command_min_secs() -> u16 {
    10
}

pub(super) fn default_toast_max_visible() -> u8 {
    3
}

pub(super) fn default_notify_sound_name() -> String {
    "system".to_string()
}

pub(super) fn default_bell_style() -> String {
    "none".to_string()
}

pub(super) fn default_zmodem_auto_detect_enabled() -> bool {
    true
}

pub(super) fn default_profile_sort() -> String {
    "default".to_string()
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
            terminal_shell_id: String::new(),
            terminal_shell_wsl_distro: String::new(),
            local_shells: Vec::new(),
            hidden_shells: Vec::new(),
            secret_vault_enabled: default_secret_vault_enabled(),
            secret_storage_mode: default_secret_storage_mode(),
            prompt_unlock_vault_on_startup: false,
            scrollback_lines: default_scrollback_lines(),
            terminal_renderer: default_terminal_renderer(),
            hidden_renderer_release_secs: default_hidden_renderer_release_secs(),
            terminal_padding_left_px: default_terminal_padding_left_px(),
            terminal_padding_right_px: default_terminal_padding_right_px(),
            terminal_padding_bottom_px: default_terminal_padding_bottom_px(),
            startup_session_restore_mode: default_startup_session_restore_mode(),
            restore_terminal_content: true,
            show_jump_host_connection_info: default_show_jump_host_connection_info(),
            sftp_paste_upload_enabled: false,
            sftp_start_directory: default_sftp_start_directory(),
            sftp_external_editor: String::new(),
            sftp_external_upload: default_sftp_external_upload(),
            monitor_refresh_interval_secs: default_monitor_refresh_interval_secs(),
            monitor_visible_metrics: default_monitor_visible_metrics(),
            update_channel: default_update_channel(),
            auto_download_updates: default_auto_download_updates(),
            update_check_frequency: default_update_check_frequency(),
            tab_width_mode: default_tab_width_mode(),
            tab_standard_width: default_tab_standard_width(),
            profile_sort: default_profile_sort(),
            tab_hover_tooltip: false,
            terminal_log_enabled: false,
            terminal_log_directory: String::new(),
            terminal_log_format: default_terminal_log_format(),
            terminal_log_name_template: default_terminal_log_name_template(),
            terminal_log_max_file_size_mb: default_terminal_log_max_file_size_mb(),
            terminal_log_compress: false,
            terminal_log_plain_timestamps: true,
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
            password_feedback: default_password_feedback(),
            predictive_echo: default_predictive_echo(),
            predictive_echo_threshold_ms: default_predictive_echo_threshold_ms(),
            mac_option_is_meta: false,
            confirm_multiline_paste: default_confirm_multiline_paste(),
            copy_on_select: false,
            right_click_paste: default_right_click_paste(),
            osc52_clipboard_write: default_osc52_clipboard_write(),
            command_marks: default_command_marks(),
            local_unix_shell_integration: default_local_unix_shell_integration(),
            legacy_local_shell_integration: false,
            ssh_shell_integration: default_ssh_shell_integration(),
            terminal_line_height: default_terminal_line_height(),
            terminal_letter_spacing: 0,
            keymap: KeymapConfig::default(),
            background_throttling: default_background_throttling(),
            window_blur: false,
            window_blur_radius: default_window_blur_radius(),
            window_opacity_percent: default_window_opacity_percent(),
            window_blur_material: default_window_blur_material(),
            ui_style: "islands".to_string(),
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
            notify_sound_name: default_notify_sound_name(),
            notify_sound_file: String::new(),
            bell_style: default_bell_style(),
            tab_icons: tab_icons::TabIconConfig::default(),
            tab_agent_indicator: tab_agent_indicator::TabAgentIndicatorConfig::default(),
            keyword_highlight: keyword_highlight::KeywordHighlightConfig::default(),
            panes: panes::PaneConfig::default(),
            default_proxy_id: String::new(),
        }
    }
}

pub(super) fn default_theme_light() -> String {
    "light".to_string()
}

pub(super) fn default_theme_dark() -> String {
    "default".to_string()
}
