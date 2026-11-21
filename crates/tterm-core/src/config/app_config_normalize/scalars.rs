//! Rules applied to individual settings after the frontend's default-value spread.

use super::{normalize_terminal_line_height, normalize_ui_scale_percent};
use crate::config::{AppConfig, limits, sftp_start_directory::normalize_sftp_start_directory};
use crate::ssh::secret_store::types::SecretStorageMode;
use serde_json::Value;
use tterm_terminal::{js_text::js_trim, theme::js::js_round};

pub(super) fn flags(config: &mut AppConfig, input: &Value) {
    macro_rules! flags {
        ($($field:ident),+) => {$(config.$field = input[stringify!($field)].as_bool().unwrap_or(config.$field);)+};
    }
    flags!(
        theme_follow_system,
        restore_terminal_content,
        resume_agent_sessions,
        show_jump_host_connection_info,
        sftp_paste_upload_enabled,
        prompt_unlock_vault_on_startup,
        auto_download_updates,
        terminal_log_enabled,
        terminal_log_compress,
        terminal_log_plain_timestamps,
        terminal_log_record_input,
        reconnect_enabled,
        zmodem_auto_detect_enabled,
        password_feedback,
        mac_option_is_meta,
        confirm_multiline_paste,
        copy_on_select,
        right_click_paste,
        osc52_clipboard_write,
        command_marks,
        local_unix_shell_integration,
        ssh_shell_integration,
        window_blur,
        notifications_enabled,
        notify_command_finished,
        notify_terminal_requests,
        notify_agent_waiting,
        notify_agent_done,
        bell_notify,
        notify_sound
    );
}

pub(super) fn choices(config: &mut AppConfig, input: &Value) {
    let text = |key: &str| input[key].as_str().unwrap_or_default();
    macro_rules! choice {
        ($field:ident, $default:literal, $($allowed:literal)|+) => {
            let saved = text(stringify!($field));
            config.$field = if matches!(saved, $($allowed)|+) { saved } else { $default }.to_owned();
        };
    }
    choice!(startup_session_restore_mode, "active", "all");
    choice!(sftp_external_upload, "ask", "auto");
    choice!(terminal_renderer, "webgl", "canvas");
    choice!(update_channel, "stable", "beta-dev");
    choice!(
        update_check_frequency,
        "daily",
        "every-3-days" | "weekly" | "never"
    );
    choice!(terminal_log_format, "both", "raw" | "plain");
    choice!(predictive_echo, "auto", "always" | "off");
    choice!(background_throttling, "throttle", "disabled" | "suspend");
    choice!(window_blur_material, "acrylic", "mica");
    choice!(ui_style, "classic", "islands");
    choice!(profile_sort, "default", "recent");
    choice!(close_behavior, "ask", "tray" | "quit");
    choice!(bell_style, "none", "visual" | "sound");
    config.secret_storage_mode = SecretStorageMode::from_config_value(text("secret_storage_mode"))
        .as_str()
        .to_owned();
    config.sftp_start_directory =
        normalize_sftp_start_directory(input["sftp_start_directory"].as_str())
            .map_or("last", |id| id.as_str())
            .to_owned();
    for (key, field) in [
        ("theme_light", &mut config.theme_light),
        ("theme_dark", &mut config.theme_dark),
    ] {
        if !text(key).is_empty() {
            *field = text(key).to_owned();
        }
    }
    if limits::NOTIFICATION_SOUND_NAMES.contains(&text("notify_sound_name")) {
        config.notify_sound_name = text("notify_sound_name").to_owned();
    }
    config.sftp_external_editor = js_trim(text("sftp_external_editor")).to_owned();
    config.default_proxy_id = js_trim(text("default_proxy_id")).to_owned();
    config.zmodem_download_directory = text("zmodem_download_directory").to_owned();
    config.terminal_log_directory = text("terminal_log_directory").to_owned();
    config.notify_sound_file = text("notify_sound_file").to_owned();
    let template = text("terminal_log_name_template");
    if !js_trim(template).is_empty() {
        config.terminal_log_name_template = template.to_owned();
    }
}

pub(super) fn numbers(config: &mut AppConfig, input: &Value) {
    macro_rules! rounded {
        ($field:ident, $min:expr, $max:expr) => {
            if let Some(number) = input[stringify!($field)].as_f64() {
                config.$field = js_round(number).clamp(f64::from($min), f64::from($max)) as _;
            }
        };
    }
    macro_rules! ranged {
        ($field:ident, $range:expr) => {
            rounded!($field, *$range.start(), *$range.end());
        };
    }
    rounded!(monitor_refresh_interval_secs, 1, 60);
    rounded!(terminal_padding_left_px, 0, 80);
    rounded!(terminal_padding_right_px, 0, 80);
    rounded!(terminal_padding_bottom_px, 0, 80);
    rounded!(terminal_log_max_file_size_mb, 1, 1024);
    rounded!(
        terminal_log_retention_days,
        0,
        limits::TERMINAL_LOG_MAX_RETENTION_DAYS
    );
    rounded!(
        terminal_log_max_total_mb,
        0,
        limits::TERMINAL_LOG_MAX_TOTAL_MB
    );
    rounded!(sftp_transfer_parallelism, 1, 16);
    rounded!(sftp_upload_limit_kib, 0, limits::MAX_BANDWIDTH_LIMIT_KIB);
    rounded!(sftp_download_limit_kib, 0, limits::MAX_BANDWIDTH_LIMIT_KIB);
    rounded!(
        reconnect_max_attempts,
        limits::RECONNECT_MAX_ATTEMPTS_MIN,
        limits::RECONNECT_MAX_ATTEMPTS_MAX
    );
    rounded!(
        predictive_echo_threshold_ms,
        limits::PREDICTIVE_ECHO_THRESHOLD_MIN_MS,
        limits::PREDICTIVE_ECHO_THRESHOLD_MAX_MS
    );
    ranged!(
        terminal_letter_spacing,
        limits::TERMINAL_LETTER_SPACING_RANGE
    );
    ranged!(window_blur_radius, limits::WINDOW_BLUR_RADIUS_RANGE);
    ranged!(window_opacity_percent, limits::WINDOW_OPACITY_PERCENT_RANGE);
    ranged!(
        notify_command_min_secs,
        limits::NOTIFY_COMMAND_MIN_SECS_RANGE
    );
    ranged!(toast_max_visible, limits::TOAST_MAX_VISIBLE_RANGE);
    if let Some(delay) = limits::HIDDEN_RENDERER_RELEASE_SECS_OPTIONS
        .into_iter()
        .find(|delay| Some(f64::from(*delay)) == input["hidden_renderer_release_secs"].as_f64())
    {
        config.hidden_renderer_release_secs = delay;
    }
    config.ui_scale_percent = normalize_ui_scale_percent(input["ui_scale_percent"].as_f64());
    config.terminal_line_height =
        normalize_terminal_line_height(input["terminal_line_height"].as_f64());
}
