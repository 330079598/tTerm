import {
  type SecretBackendStatus,
  type SecretStorageMode,
  getDefaultTerminalRenderer,
  HIDDEN_RENDERER_RELEASE_SECS_OPTIONS,
  NOTIFICATION_SOUND_NAMES,
  type NotificationSoundName,
  DEFAULT_HIDDEN_RENDERER_RELEASE_SECS,
  type AppConfig,
  defaultConfig,
  TERMINAL_LOG_MAX_RETENTION_DAYS,
  TERMINAL_LOG_MAX_TOTAL_MB,
  MAX_BANDWIDTH_LIMIT_KIB,
  RECONNECT_MAX_ATTEMPTS_MIN,
  DEFAULT_PREDICTIVE_ECHO_THRESHOLD_MS,
  PREDICTIVE_ECHO_THRESHOLD_MIN_MS,
  PREDICTIVE_ECHO_THRESHOLD_MAX_MS,
  RECONNECT_MAX_ATTEMPTS_MAX,
  TERMINAL_LINE_HEIGHT_RANGE,
  TERMINAL_LETTER_SPACING_RANGE,
  WINDOW_OPACITY_PERCENT_RANGE,
  WINDOW_BLUR_RADIUS_RANGE,
  NOTIFY_COMMAND_MIN_SECS_RANGE,
  TOAST_MAX_VISIBLE_RANGE,
  type AppState,
} from "@/lib/appConfig"
import { normalizeVerificationMethod } from "@/lib/userVerification"
import type { UpdateCheckFrequency } from "@/lib/updater"
import { normalizeKeymap } from "@/lib/keymap/keymap"
import { normalizeKeywordHighlightConfig } from "@/lib/keywordHighlight/config"
import { normalizeMonitorVisibleMetrics } from "@/lib/monitorMetrics"
import { normalizeSftpStartDirectory } from "@/lib/sftpStartDirectory"
import { normalizeTabSettings } from "@/lib/tabSettings"

function normalizeHiddenRendererReleaseSecs(value: unknown): number {
  return (HIDDEN_RENDERER_RELEASE_SECS_OPTIONS as readonly unknown[]).includes(value)
    ? (value as number)
    : DEFAULT_HIDDEN_RENDERER_RELEASE_SECS
}

function normalizeNotificationSoundName(value: unknown): NotificationSoundName {
  return (NOTIFICATION_SOUND_NAMES as readonly unknown[]).includes(value)
    ? (value as NotificationSoundName)
    : "system"
}

/** Also accepts the modes used before passwords moved into the database. */
function normalizeSecretStorageMode(mode: unknown): SecretStorageMode {
  if (mode === "password" || mode === "vault") return "password"
  if (mode === "memory") return "memory"
  return "system"
}

function normalizeUpdateCheckFrequency(
  frequency: Partial<AppConfig>["update_check_frequency"]
): UpdateCheckFrequency {
  if (frequency === "every-3-days" || frequency === "weekly" || frequency === "never") {
    return frequency
  }

  return "daily"
}

function normalizeTerminalPadding(
  value: Partial<AppConfig>["terminal_padding_left_px"],
  fallback: number
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback
  }

  return Math.min(Math.max(Math.round(value), 0), 80)
}

function normalizeMonitorRefreshInterval(
  value: Partial<AppConfig>["monitor_refresh_interval_secs"]
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 5
  }

  return Math.min(Math.max(Math.round(value), 1), 60)
}

function normalizeAgentConfigDirs(
  value: Partial<AppConfig>["agent_config_dirs"]
): Record<string, string> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return {}
  }

  return Object.fromEntries(
    Object.entries(value).filter(
      (entry): entry is [string, string] => typeof entry[1] === "string" && entry[1] !== ""
    )
  )
}

function normalizeTerminalLogFileSize(
  value: Partial<AppConfig>["terminal_log_max_file_size_mb"]
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 50
  }
  return Math.min(Math.max(Math.round(value), 1), 1024)
}

/** A whole number in `0..=max`; anything else is 0 (off). */
function normalizeTerminalLogLimit(value: number | undefined, max: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    return 0
  }
  return Math.min(Math.round(value), max)
}

function normalizeSftpTransferParallelism(
  value: Partial<AppConfig>["sftp_transfer_parallelism"]
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 4
  }
  return Math.min(Math.max(Math.round(value), 1), 16)
}

/** Upper bound for an SFTP bandwidth limit (10 GB/s), well inside `u32` KiB. */

function normalizeBandwidthLimitKib(value: Partial<AppConfig>["sftp_upload_limit_kib"]): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 0
  }
  return Math.min(Math.max(Math.round(value), 0), MAX_BANDWIDTH_LIMIT_KIB)
}

function normalizeReconnectMaxAttempts(
  value: Partial<AppConfig>["reconnect_max_attempts"]
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 5
  }
  return Math.min(
    Math.max(Math.round(value), RECONNECT_MAX_ATTEMPTS_MIN),
    RECONNECT_MAX_ATTEMPTS_MAX
  )
}

function normalizePredictiveEchoThresholdMs(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return DEFAULT_PREDICTIVE_ECHO_THRESHOLD_MS
  }
  return Math.min(
    Math.max(Math.round(value), PREDICTIVE_ECHO_THRESHOLD_MIN_MS),
    PREDICTIVE_ECHO_THRESHOLD_MAX_MS
  )
}

export function normalizeUiScalePercent(value: Partial<AppConfig>["ui_scale_percent"]): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 100
  }

  return Math.min(Math.max(Math.round(value / 10) * 10, 80), 200)
}

export function normalizeTerminalLineHeight(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 1
  }
  return Math.min(
    Math.max(Math.round(value * 100) / 100, TERMINAL_LINE_HEIGHT_RANGE.min),
    TERMINAL_LINE_HEIGHT_RANGE.max
  )
}

function normalizeRoundedInRange(
  value: unknown,
  range: { min: number; max: number },
  fallback: number
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback
  }

  return Math.min(Math.max(Math.round(value), range.min), range.max)
}

export function normalizeConfig(config: Partial<AppConfig>): AppConfig {
  return {
    ...defaultConfig,
    ...config,
    theme_follow_system: config.theme_follow_system === true,
    theme_light:
      typeof config.theme_light === "string" && config.theme_light ? config.theme_light : "light",
    theme_dark:
      typeof config.theme_dark === "string" && config.theme_dark ? config.theme_dark : "default",
    favorite_themes: Array.isArray(config.favorite_themes)
      ? [...new Set(config.favorite_themes.filter((id) => typeof id === "string" && id))]
      : [],
    startup_session_restore_mode: config.startup_session_restore_mode === "all" ? "all" : "active",
    restore_terminal_content: config.restore_terminal_content !== false,
    show_jump_host_connection_info: config.show_jump_host_connection_info !== false,
    sftp_paste_upload_enabled: config.sftp_paste_upload_enabled === true,
    sftp_start_directory: normalizeSftpStartDirectory(config.sftp_start_directory) ?? "last",
    secret_storage_mode: normalizeSecretStorageMode(config.secret_storage_mode),
    prompt_unlock_vault_on_startup: config.prompt_unlock_vault_on_startup === true,
    terminal_renderer:
      config.terminal_renderer === "canvas" || config.terminal_renderer === "webgl"
        ? config.terminal_renderer
        : getDefaultTerminalRenderer(),
    hidden_renderer_release_secs: normalizeHiddenRendererReleaseSecs(
      config.hidden_renderer_release_secs
    ),
    monitor_refresh_interval_secs: normalizeMonitorRefreshInterval(
      config.monitor_refresh_interval_secs
    ),
    monitor_visible_metrics: normalizeMonitorVisibleMetrics(config.monitor_visible_metrics),
    update_channel: config.update_channel === "beta-dev" ? "beta-dev" : "stable",
    auto_download_updates: config.auto_download_updates !== false,
    terminal_padding_left_px: normalizeTerminalPadding(config.terminal_padding_left_px, 6),
    terminal_padding_right_px: normalizeTerminalPadding(config.terminal_padding_right_px, 6),
    terminal_padding_bottom_px: normalizeTerminalPadding(config.terminal_padding_bottom_px, 4),
    update_check_frequency: normalizeUpdateCheckFrequency(config.update_check_frequency),
    ...normalizeTabSettings(config),
    terminal_log_enabled: config.terminal_log_enabled === true,
    terminal_log_directory:
      typeof config.terminal_log_directory === "string" ? config.terminal_log_directory : "",
    terminal_log_format:
      config.terminal_log_format === "raw" || config.terminal_log_format === "plain"
        ? config.terminal_log_format
        : "both",
    terminal_log_name_template:
      typeof config.terminal_log_name_template === "string" &&
      config.terminal_log_name_template.trim()
        ? config.terminal_log_name_template
        : "{year}-{month}-{day}/{profile}-{host}-{time}-{sessionId}",
    terminal_log_max_file_size_mb: normalizeTerminalLogFileSize(
      config.terminal_log_max_file_size_mb
    ),
    terminal_log_compress: config.terminal_log_compress === true,
    terminal_log_plain_timestamps: config.terminal_log_plain_timestamps !== false,
    terminal_log_record_input: config.terminal_log_record_input === true,
    terminal_log_retention_days: normalizeTerminalLogLimit(
      config.terminal_log_retention_days,
      TERMINAL_LOG_MAX_RETENTION_DAYS
    ),
    terminal_log_max_total_mb: normalizeTerminalLogLimit(
      config.terminal_log_max_total_mb,
      TERMINAL_LOG_MAX_TOTAL_MB
    ),
    sftp_transfer_parallelism: normalizeSftpTransferParallelism(config.sftp_transfer_parallelism),
    sftp_upload_limit_kib: normalizeBandwidthLimitKib(config.sftp_upload_limit_kib),
    sftp_download_limit_kib: normalizeBandwidthLimitKib(config.sftp_download_limit_kib),
    reconnect_enabled: config.reconnect_enabled !== false,
    reconnect_max_attempts: normalizeReconnectMaxAttempts(config.reconnect_max_attempts),
    zmodem_auto_detect_enabled: config.zmodem_auto_detect_enabled !== false,
    zmodem_download_directory:
      typeof config.zmodem_download_directory === "string" ? config.zmodem_download_directory : "",
    default_proxy_id:
      typeof config.default_proxy_id === "string" ? config.default_proxy_id.trim() : "",
    sudo_prompt_patterns: Array.isArray(config.sudo_prompt_patterns)
      ? config.sudo_prompt_patterns.filter(
          (pattern): pattern is string => typeof pattern === "string" && pattern.trim() !== ""
        )
      : [],
    password_feedback: config.password_feedback !== false,
    predictive_echo:
      config.predictive_echo === "always" || config.predictive_echo === "off"
        ? config.predictive_echo
        : "auto",
    predictive_echo_threshold_ms: normalizePredictiveEchoThresholdMs(
      config.predictive_echo_threshold_ms
    ),
    ui_scale_percent: normalizeUiScalePercent(config.ui_scale_percent),
    mac_option_is_meta: config.mac_option_is_meta === true,
    confirm_multiline_paste: config.confirm_multiline_paste !== false,
    copy_on_select: config.copy_on_select === true,
    right_click_paste:
      typeof config.right_click_paste === "boolean"
        ? config.right_click_paste
        : defaultConfig.right_click_paste,
    osc52_clipboard_write: config.osc52_clipboard_write !== false,
    command_marks: config.command_marks !== false,
    keyword_highlight: normalizeKeywordHighlightConfig(config.keyword_highlight),
    local_shell_integration: config.local_shell_integration === true,
    terminal_line_height: normalizeTerminalLineHeight(config.terminal_line_height),
    terminal_letter_spacing: normalizeRoundedInRange(
      config.terminal_letter_spacing,
      TERMINAL_LETTER_SPACING_RANGE,
      0
    ),
    keymap: normalizeKeymap(config.keymap),
    background_throttling:
      config.background_throttling === "disabled" || config.background_throttling === "suspend"
        ? config.background_throttling
        : "throttle",
    window_blur: config.window_blur === true,
    window_blur_radius: normalizeRoundedInRange(
      config.window_blur_radius,
      WINDOW_BLUR_RADIUS_RANGE,
      20
    ),
    window_opacity_percent: normalizeRoundedInRange(
      config.window_opacity_percent,
      WINDOW_OPACITY_PERCENT_RANGE,
      70
    ),
    window_blur_material: config.window_blur_material === "mica" ? "mica" : "acrylic",
    ui_style: config.ui_style === "islands" ? "islands" : "classic",
    profile_sort: config.profile_sort === "recent" ? "recent" : "default",
    close_behavior:
      config.close_behavior === "tray" || config.close_behavior === "quit"
        ? config.close_behavior
        : "ask",
    notifications_enabled: config.notifications_enabled !== false,
    notify_command_finished: config.notify_command_finished !== false,
    notify_command_min_secs: normalizeRoundedInRange(
      config.notify_command_min_secs,
      NOTIFY_COMMAND_MIN_SECS_RANGE,
      10
    ),
    notify_terminal_requests: config.notify_terminal_requests !== false,
    notify_agent_waiting: config.notify_agent_waiting !== false,
    notify_agent_done: config.notify_agent_done !== false,
    agent_config_dirs: normalizeAgentConfigDirs(config.agent_config_dirs),
    bell_notify: config.bell_notify === true,
    toast_max_visible: normalizeRoundedInRange(
      config.toast_max_visible,
      TOAST_MAX_VISIBLE_RANGE,
      3
    ),
    notify_sound: config.notify_sound !== false,
    notify_sound_name: normalizeNotificationSoundName(config.notify_sound_name),
    notify_sound_file: typeof config.notify_sound_file === "string" ? config.notify_sound_file : "",
    bell_style:
      config.bell_style === "visual" || config.bell_style === "sound" ? config.bell_style : "none",
  }
}

export function normalizeAppState(state: Partial<AppState>): AppState {
  return {
    lastUpdateCheckAt: typeof state.lastUpdateCheckAt === "number" ? state.lastUpdateCheckAt : null,
    collapsedProfileGroupKeys: Array.isArray(state.collapsedProfileGroupKeys)
      ? state.collapsedProfileGroupKeys.filter((item): item is string => typeof item === "string")
      : [],
  }
}

export function normalizeSecretStatus(status?: Partial<SecretBackendStatus>): SecretBackendStatus {
  return {
    storageMode: normalizeSecretStorageMode(status?.storageMode),
    keyringAvailable: status?.keyringAvailable ?? false,
    unlocked: status?.unlocked ?? false,
    hasMasterPassword: status?.hasMasterPassword ?? false,
    persistenceAvailable: status?.persistenceAvailable ?? false,
    migrationPending: status?.migrationPending ?? false,
    verificationMethod: normalizeVerificationMethod(status?.verificationMethod),
    message: status?.message ?? null,
  }
}
