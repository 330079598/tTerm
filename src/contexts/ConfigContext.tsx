import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react"
import { invoke } from "@tauri-apps/api/core"
import { platform } from "@tauri-apps/plugin-os"

import { detectSystemLanguage } from "@/i18n/language"
import { markConfigReady } from "@/lib/startup"
import { onSyncApplied } from "@/lib/sync"
import { setWindowBlur } from "@/lib/themePreloader"
import { normalizeVerificationMethod, type VerificationMethod } from "@/lib/userVerification"
import type { UpdateCheckFrequency } from "@/lib/updater"
import { DEFAULT_KEYMAP_CONFIG, normalizeKeymap, type KeymapConfig } from "@/lib/keymap/keymap"

export interface SecretBackendStatus {
  storageMode: SecretStorageMode
  keyringAvailable: boolean
  /** Saved passwords can be read and written right now. */
  unlocked: boolean
  hasMasterPassword: boolean
  persistenceAvailable: boolean
  /** Passwords in the old app vault wait for its password to be moved. */
  migrationPending: boolean
  /** How showing or exporting saved passwords is confirmed. */
  verificationMethod: VerificationMethod
  message?: string | null
}

/** Where the key that unlocks saved passwords comes from. */
export type SecretStorageMode = "system" | "password" | "memory"
export type TabWidthMode = "adaptive" | "standard"
export type TerminalLogFormat = "raw" | "plain" | "both"
export type TerminalRenderer = "webgl" | "canvas"
/** What WebKit does with the page while the window is hidden (macOS 14+). */
export type BackgroundThrottling = "throttle" | "disabled" | "suspend"
/** What closing the main window does; "tray" keeps tTerm running in the background. */
export type CloseBehavior = "ask" | "tray" | "quit"
export type WindowBlurMaterial = "acrylic" | "mica"
/** What the terminal bell does: flash the terminal, play a tone, or nothing. */
export type BellStyle = "visual" | "sound" | "none"
export type MonitorMetricId =
  "cpu" | "memory" | "network" | "ip" | "latency" | "disk" | "load" | "uptime"

export const DEFAULT_MONITOR_VISIBLE_METRICS: MonitorMetricId[] = [
  "cpu",
  "memory",
  "network",
  "ip",
  "latency",
  "disk",
]

const MONITOR_METRIC_IDS = new Set<MonitorMetricId>([
  ...DEFAULT_MONITOR_VISIBLE_METRICS,
  "load",
  "uptime",
])

export function applyUiScalePercent(scale: number) {
  const rootStyle = document.documentElement.style
  const factor = scale / 100
  rootStyle.setProperty("--ui-font-scale", String(factor))
  rootStyle.setProperty("--text-xs", `${0.75 * factor}rem`)
  rootStyle.setProperty("--text-xs--line-height", "1.15")
  rootStyle.setProperty("--text-sm", `${0.875 * factor}rem`)
  rootStyle.setProperty("--text-sm--line-height", "1.2")
  rootStyle.setProperty("--text-base", `${factor}rem`)
  rootStyle.setProperty("--text-base--line-height", "1.25")
  rootStyle.setProperty("--text-lg", `${1.125 * factor}rem`)
  rootStyle.setProperty("--text-lg--line-height", "1.25")
  rootStyle.setProperty("--text-xl", `${1.25 * factor}rem`)
  rootStyle.setProperty("--text-xl--line-height", "1.25")
  rootStyle.setProperty("--text-2xl", `${1.5 * factor}rem`)
  rootStyle.setProperty("--text-2xl--line-height", "1.2")
  rootStyle.setProperty("--text-3xl", `${1.875 * factor}rem`)
  rootStyle.setProperty("--text-3xl--line-height", "1.2")
}

let _cachedPlatform: string | null = null

export function getDetectedPlatform(): string {
  try {
    _cachedPlatform ??= platform()
    return _cachedPlatform
  } catch {
    if (typeof navigator !== "undefined") {
      const hint = `${navigator.platform} ${navigator.userAgent}`.toLowerCase()
      if (hint.includes("mac")) return "macos"
      if (hint.includes("win")) return "windows"
      if (hint.includes("linux")) return "linux"
    }
    return "unknown"
  }
}

export function getDefaultTerminalRenderer(): TerminalRenderer {
  return "webgl"
}

/** How long a hidden terminal keeps its renderer; 0 keeps it for good. */
export const HIDDEN_RENDERER_RELEASE_SECS_OPTIONS = [0, 10, 30, 60, 300] as const
const DEFAULT_HIDDEN_RENDERER_RELEASE_SECS = 10

function normalizeHiddenRendererReleaseSecs(value: unknown): number {
  return (HIDDEN_RENDERER_RELEASE_SECS_OPTIONS as readonly unknown[]).includes(value)
    ? (value as number)
    : DEFAULT_HIDDEN_RENDERER_RELEASE_SECS
}

/** Also accepts the modes used before passwords moved into the database. */
function normalizeSecretStorageMode(mode: unknown): SecretStorageMode {
  if (mode === "password" || mode === "vault") return "password"
  if (mode === "memory") return "memory"
  return "system"
}

export interface SavedSecretEntry {
  key: string
  profileId: string
  profileName: string
  label: string
  kind: string
}

export interface AppConfig {
  theme: string
  /** Show `theme_light` or `theme_dark` with the system's appearance instead of `theme`. */
  theme_follow_system: boolean
  theme_light: string
  theme_dark: string
  /** Theme ids starred in the theme library. */
  favorite_themes: string[]
  language: string
  font_family: string
  font_size: number
  ui_scale_percent: number
  cursor_style: "bar" | "block" | "underline"
  terminal_shell: "auto" | "cmd" | "powershell" | "pwsh" | "wsl" | "git-bash" | "custom"
  terminal_shell_custom_path: string
  terminal_shell_custom_args: string
  secret_vault_enabled: boolean
  secret_storage_mode: SecretStorageMode
  prompt_unlock_vault_on_startup: boolean
  scrollback_lines: number
  terminal_renderer: TerminalRenderer
  hidden_renderer_release_secs: number
  terminal_padding_left_px: number
  terminal_padding_right_px: number
  terminal_padding_bottom_px: number
  startup_session_restore_mode: "active" | "all"
  show_jump_host_connection_info: boolean
  sftp_paste_upload_enabled: boolean
  monitor_refresh_interval_secs: number
  monitor_visible_metrics: MonitorMetricId[]
  update_channel: "stable" | "beta-dev"
  auto_download_updates: boolean
  update_check_frequency: UpdateCheckFrequency
  tab_width_mode: TabWidthMode
  tab_standard_width: number
  terminal_log_enabled: boolean
  terminal_log_directory: string
  terminal_log_format: TerminalLogFormat
  terminal_log_name_template: string
  terminal_log_max_file_size_mb: number
  terminal_log_compress: boolean
  sftp_transfer_parallelism: number
  /** SFTP upload bandwidth cap in KiB/s shared by all transfers; 0 = unlimited. */
  sftp_upload_limit_kib: number
  /** SFTP download bandwidth cap in KiB/s shared by all transfers; 0 = unlimited. */
  sftp_download_limit_kib: number
  reconnect_enabled: boolean
  reconnect_max_attempts: number
  zmodem_auto_detect_enabled: boolean
  zmodem_download_directory: string
  /** Extra sudo password prompt regexes; an optional `user` group names the account. */
  sudo_prompt_patterns: string[]
  /** macOS: Option+key sends Meta (ESC prefix) instead of the alternate character. */
  mac_option_is_meta: boolean
  /** Ask before pasting multiple lines into a shell without bracketed paste mode. */
  confirm_multiline_paste: boolean
  /** Copy the terminal selection to the clipboard when the mouse button is released. */
  copy_on_select: boolean
  /** Right-click pastes instead of opening the menu; Shift+right-click still opens it. */
  right_click_paste: boolean
  /** Local shells mark their prompts and commands (OSC 133) for jumping and copying output. */
  command_marks: boolean
  /** macOS and Linux: local bash, zsh and fish start through tTerm's shell integration. */
  local_shell_integration: boolean
  /** Multiple of the font's cell height. */
  terminal_line_height: number
  /** Extra pixels between characters; may be negative. */
  terminal_letter_spacing: number
  keymap: KeymapConfig
  background_throttling: BackgroundThrottling
  /** Blur what is behind the window (macOS, Windows), tinted with the theme background. */
  window_blur: boolean
  /** Blur radius in points (macOS). */
  window_blur_radius: number
  /** Windows backdrop: acrylic blurs what is behind, mica tints with the wallpaper. */
  window_blur_material: WindowBlurMaterial
  /** Opacity of that tint in percent. */
  window_opacity_percent: number
  close_behavior: CloseBehavior
  /** Announce terminal events while the user is elsewhere (system or in-app). */
  notifications_enabled: boolean
  /** Announce commands that ran at least `notify_command_min_secs` (needs shell integration). */
  notify_command_finished: boolean
  notify_command_min_secs: number
  /** Announce notifications programs ask for (OSC 9, 777, 99). */
  notify_terminal_requests: boolean
  /** Announce an AI agent waiting for the user (tTerm's agent hooks). */
  notify_agent_waiting: boolean
  /** Announce an AI agent finishing its turn or stopping on an error. */
  notify_agent_done: boolean
  /** AI agents' config directories chosen by hand, by agent; others are found. */
  agent_config_dirs: Record<string, string>
  /** Announce a bell in the background with a system notification. */
  bell_notify: boolean
  /** In-app toasts stacked at once; a new one closes the oldest past this. */
  toast_max_visible: number
  /** System notifications play the system sound. */
  notify_sound: boolean
  bell_style: BellStyle
}

const defaultUpdateChannel = /-(alpha|beta|rc|dev)(\.|$)/.test(
  import.meta.env.PACKAGE_VERSION ?? ""
)
  ? "beta-dev"
  : "stable"

export const DEFAULT_TERMINAL_FONT_SIZE = 14

export const DEFAULT_TERMINAL_FONT_FAMILY =
  '"JetBrains Mono Nerd Font", "JetBrainsMono Nerd Font", "JetBrains Mono", "Fira Code", Menlo, Monaco, monospace'

const defaultConfig: AppConfig = {
  theme: "default",
  theme_follow_system: false,
  theme_light: "light",
  theme_dark: "default",
  favorite_themes: [],
  language: detectSystemLanguage(),
  font_family: DEFAULT_TERMINAL_FONT_FAMILY,
  font_size: DEFAULT_TERMINAL_FONT_SIZE,
  ui_scale_percent: 100,
  cursor_style: "block",
  terminal_shell: "auto",
  terminal_shell_custom_path: "",
  terminal_shell_custom_args: "",
  secret_vault_enabled: true,
  secret_storage_mode: "system",
  prompt_unlock_vault_on_startup: false,
  scrollback_lines: 10000,
  terminal_renderer: getDefaultTerminalRenderer(),
  hidden_renderer_release_secs: DEFAULT_HIDDEN_RENDERER_RELEASE_SECS,
  terminal_padding_left_px: 6,
  terminal_padding_right_px: 0,
  terminal_padding_bottom_px: 0,
  startup_session_restore_mode: "active",
  show_jump_host_connection_info: true,
  sftp_paste_upload_enabled: false,
  monitor_refresh_interval_secs: 5,
  monitor_visible_metrics: DEFAULT_MONITOR_VISIBLE_METRICS,
  update_channel: defaultUpdateChannel,
  auto_download_updates: true,
  update_check_frequency: "daily",
  tab_width_mode: "adaptive",
  tab_standard_width: 120,
  terminal_log_enabled: false,
  terminal_log_directory: "",
  terminal_log_format: "both",
  terminal_log_name_template: "{profile}-{host}-{yyyyMMdd-HHmmss}-{sessionId}",
  terminal_log_max_file_size_mb: 50,
  terminal_log_compress: false,
  sftp_transfer_parallelism: 4,
  sftp_upload_limit_kib: 0,
  sftp_download_limit_kib: 0,
  reconnect_enabled: true,
  reconnect_max_attempts: 5,
  zmodem_auto_detect_enabled: true,
  zmodem_download_directory: "",
  sudo_prompt_patterns: [],
  mac_option_is_meta: false,
  confirm_multiline_paste: true,
  copy_on_select: false,
  right_click_paste: false,
  command_marks: true,
  local_shell_integration: false,
  terminal_line_height: 1,
  terminal_letter_spacing: 0,
  keymap: { ...DEFAULT_KEYMAP_CONFIG, bindings: {} },
  background_throttling: "throttle",
  window_blur: false,
  window_blur_radius: 20,
  window_opacity_percent: 70,
  window_blur_material: "acrylic",
  close_behavior: "ask",
  notifications_enabled: true,
  notify_command_finished: true,
  notify_command_min_secs: 10,
  notify_terminal_requests: true,
  notify_agent_waiting: true,
  notify_agent_done: true,
  agent_config_dirs: {},
  bell_notify: false,
  toast_max_visible: 3,
  notify_sound: true,
  bell_style: "none",
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

function normalizeMonitorVisibleMetrics(
  value: Partial<AppConfig>["monitor_visible_metrics"]
): MonitorMetricId[] {
  if (!Array.isArray(value)) {
    return [...DEFAULT_MONITOR_VISIBLE_METRICS]
  }

  const metrics = value.filter(
    (item, index): item is MonitorMetricId =>
      MONITOR_METRIC_IDS.has(item as MonitorMetricId) && value.indexOf(item) === index
  )
  return metrics.length > 0 ? metrics : [...DEFAULT_MONITOR_VISIBLE_METRICS]
}

function normalizeTabStandardWidth(value: Partial<AppConfig>["tab_standard_width"]): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 120
  }

  return Math.min(Math.max(Math.round(value), 80), 300)
}

function normalizeTerminalLogFileSize(
  value: Partial<AppConfig>["terminal_log_max_file_size_mb"]
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 50
  }
  return Math.min(Math.max(Math.round(value), 1), 1024)
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
export const MAX_BANDWIDTH_LIMIT_KIB = 10 * 1024 * 1024

function normalizeBandwidthLimitKib(value: Partial<AppConfig>["sftp_upload_limit_kib"]): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 0
  }
  return Math.min(Math.max(Math.round(value), 0), MAX_BANDWIDTH_LIMIT_KIB)
}

export const RECONNECT_MAX_ATTEMPTS_MIN = 1
export const RECONNECT_MAX_ATTEMPTS_MAX = 99

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

export function normalizeUiScalePercent(value: Partial<AppConfig>["ui_scale_percent"]): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 100
  }

  return Math.min(Math.max(Math.round(value / 10) * 10, 80), 200)
}

export const TERMINAL_FONT_SIZE_RANGE = { min: 6, max: 72 } as const
export const TERMINAL_LINE_HEIGHT_RANGE = { min: 1, max: 2 } as const
export const TERMINAL_LETTER_SPACING_RANGE = { min: -5, max: 10 } as const

export function normalizeTerminalLineHeight(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 1
  }
  return Math.min(
    Math.max(Math.round(value * 100) / 100, TERMINAL_LINE_HEIGHT_RANGE.min),
    TERMINAL_LINE_HEIGHT_RANGE.max
  )
}

export const WINDOW_OPACITY_PERCENT_RANGE = { min: 30, max: 95 } as const
export const WINDOW_BLUR_RADIUS_RANGE = { min: 1, max: 60 } as const
export const NOTIFY_COMMAND_MIN_SECS_RANGE = { min: 1, max: 3600 } as const
export const TOAST_MAX_VISIBLE_RANGE = { min: 1, max: 5 } as const

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

/** The blur needs the window system's help; Linux leaves it to the compositor. */
export function isWindowBlurSupported(): boolean {
  const detected = getDetectedPlatform()
  return detected === "macos" || detected === "windows"
}

export function isWindowBlurEnabled(config: Pick<AppConfig, "window_blur">): boolean {
  return config.window_blur && isWindowBlurSupported()
}

function normalizeConfig(config: Partial<AppConfig>): AppConfig {
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
    show_jump_host_connection_info: config.show_jump_host_connection_info !== false,
    sftp_paste_upload_enabled: config.sftp_paste_upload_enabled === true,
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
    terminal_padding_right_px: normalizeTerminalPadding(config.terminal_padding_right_px, 0),
    terminal_padding_bottom_px: normalizeTerminalPadding(config.terminal_padding_bottom_px, 0),
    update_check_frequency: normalizeUpdateCheckFrequency(config.update_check_frequency),
    tab_width_mode: config.tab_width_mode === "standard" ? "standard" : "adaptive",
    tab_standard_width: normalizeTabStandardWidth(config.tab_standard_width),
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
        : "{profile}-{host}-{yyyyMMdd-HHmmss}-{sessionId}",
    terminal_log_max_file_size_mb: normalizeTerminalLogFileSize(
      config.terminal_log_max_file_size_mb
    ),
    terminal_log_compress: config.terminal_log_compress === true,
    sftp_transfer_parallelism: normalizeSftpTransferParallelism(config.sftp_transfer_parallelism),
    sftp_upload_limit_kib: normalizeBandwidthLimitKib(config.sftp_upload_limit_kib),
    sftp_download_limit_kib: normalizeBandwidthLimitKib(config.sftp_download_limit_kib),
    reconnect_enabled: config.reconnect_enabled !== false,
    reconnect_max_attempts: normalizeReconnectMaxAttempts(config.reconnect_max_attempts),
    zmodem_auto_detect_enabled: config.zmodem_auto_detect_enabled !== false,
    zmodem_download_directory:
      typeof config.zmodem_download_directory === "string" ? config.zmodem_download_directory : "",
    sudo_prompt_patterns: Array.isArray(config.sudo_prompt_patterns)
      ? config.sudo_prompt_patterns.filter(
          (pattern): pattern is string => typeof pattern === "string" && pattern.trim() !== ""
        )
      : [],
    ui_scale_percent: normalizeUiScalePercent(config.ui_scale_percent),
    mac_option_is_meta: config.mac_option_is_meta === true,
    confirm_multiline_paste: config.confirm_multiline_paste !== false,
    copy_on_select: config.copy_on_select === true,
    right_click_paste: config.right_click_paste === true,
    command_marks: config.command_marks !== false,
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
    bell_style:
      config.bell_style === "visual" || config.bell_style === "sound" ? config.bell_style : "none",
  }
}

/** State the app keeps for itself rather than settings the user chose. */
export interface AppState {
  lastUpdateCheckAt: number | null
  collapsedProfileGroupKeys: string[]
}

const defaultAppState: AppState = {
  lastUpdateCheckAt: null,
  collapsedProfileGroupKeys: [],
}

function normalizeAppState(state: Partial<AppState>): AppState {
  return {
    lastUpdateCheckAt: typeof state.lastUpdateCheckAt === "number" ? state.lastUpdateCheckAt : null,
    collapsedProfileGroupKeys: Array.isArray(state.collapsedProfileGroupKeys)
      ? state.collapsedProfileGroupKeys.filter((item): item is string => typeof item === "string")
      : [],
  }
}

let launchBackgroundThrottling: BackgroundThrottling | null = null

/**
 * The policy the window was created with. The backend reads it only when it
 * builds the window, so a saved value that differs takes a restart.
 */
export function getLaunchBackgroundThrottling(): BackgroundThrottling | null {
  return launchBackgroundThrottling
}

const defaultSecretStatus: SecretBackendStatus = {
  storageMode: "system",
  keyringAvailable: false,
  unlocked: false,
  hasMasterPassword: false,
  persistenceAvailable: false,
  migrationPending: false,
  verificationMethod: "none",
  message: null,
}

interface ConfigContextType {
  config: AppConfig
  appState: AppState
  isLoaded: boolean
  secretStatus: SecretBackendStatus
  /** Startup has unlocked saved passwords, or found it cannot, and `secretStatus` says which. */
  isSecretStatusLoaded: boolean
  updateTheme: (theme: string) => Promise<void>
  updateLanguage: (language: string) => Promise<void>
  saveConfig: (newConfig: Partial<AppConfig>) => Promise<void>
  saveAppState: (update: Partial<AppState>) => Promise<void>
  loadConfig: () => Promise<void>
  refreshSecretStatus: () => Promise<SecretBackendStatus>
  setSecretStorageMode: (mode: SecretStorageMode, password?: string) => Promise<SecretBackendStatus>
  unlockSecretVault: (password: string) => Promise<SecretBackendStatus>
  lockSecretVault: () => Promise<SecretBackendStatus>
  /** Tries the system credential store again when it did not unlock at startup. */
  retrySystemUnlock: () => Promise<SecretBackendStatus>
  /** Deletes saved passwords that can no longer be unlocked and starts over. */
  resetSavedPasswords: () => Promise<SecretBackendStatus>
  changeVaultPassword: (
    currentPassword: string,
    newPassword: string
  ) => Promise<SecretBackendStatus>
  setMasterPassword: (password: string) => Promise<SecretBackendStatus>
  removeMasterPassword: () => Promise<SecretBackendStatus>
  listSavedSecrets: () => Promise<SavedSecretEntry[]>
  getSavedSecret: (key: string) => Promise<string>
  deleteSavedSecret: (key: string) => Promise<boolean>
}

const ConfigContext = createContext<ConfigContextType | undefined>(undefined)

function normalizeSecretStatus(status?: Partial<SecretBackendStatus>): SecretBackendStatus {
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

export function ConfigProvider({ children }: { children: React.ReactNode }) {
  const [config, setConfig] = useState<AppConfig>(defaultConfig)
  const configRef = useRef(config)
  const configSaveQueueRef = useRef<Promise<void>>(Promise.resolve())
  const [appState, setAppState] = useState<AppState>(defaultAppState)
  const appStateRef = useRef(appState)
  const appStateSaveQueueRef = useRef<Promise<void>>(Promise.resolve())
  const [secretStatus, setSecretStatus] = useState<SecretBackendStatus>(defaultSecretStatus)
  const [isSecretStatusLoaded, setIsSecretStatusLoaded] = useState(false)
  const [isLoaded, setIsLoaded] = useState(false)

  const updateConfigState = useCallback((update: (current: AppConfig) => AppConfig) => {
    const updatedConfig = update(configRef.current)
    configRef.current = updatedConfig
    setConfig(updatedConfig)
  }, [])

  useEffect(() => {
    applyUiScalePercent(config.ui_scale_percent)
    return () => {
      const rootStyle = document.documentElement.style
      rootStyle.removeProperty("--ui-font-scale")
      for (const size of ["xs", "sm", "base", "lg", "xl", "2xl", "3xl"]) {
        rootStyle.removeProperty(`--text-${size}`)
        rootStyle.removeProperty(`--text-${size}--line-height`)
      }
    }
  }, [config.ui_scale_percent])

  // Not before the config loads: the page starts with whatever the window was
  // created with, and the defaults would switch the blur off in between.
  const windowBlurEnabled = isWindowBlurEnabled(config)
  useEffect(() => {
    if (!isLoaded) return
    setWindowBlur({
      enabled: windowBlurEnabled,
      radius: config.window_blur_radius,
      material: config.window_blur_material,
      opacity: config.window_opacity_percent / 100,
    })
  }, [
    config.window_blur_material,
    config.window_blur_radius,
    config.window_opacity_percent,
    isLoaded,
    windowBlurEnabled,
  ])

  const refreshSecretStatus = useCallback(async (): Promise<SecretBackendStatus> => {
    try {
      const status = await invoke<SecretBackendStatus>("get_secret_backend_status")
      const normalized = normalizeSecretStatus(status)
      setSecretStatus(normalized)
      return normalized
    } catch (error) {
      console.error("Failed to load secret backend status:", error)
      setSecretStatus(defaultSecretStatus)
      return defaultSecretStatus
    } finally {
      setIsSecretStatusLoaded(true)
    }
  }, [])

  const loadConfig = useCallback(async (): Promise<void> => {
    try {
      const [loadedConfig, loadedAppState] = await Promise.all([
        invoke<AppConfig>("load_config"),
        // The app still works on defaults when its state cannot be read.
        invoke<AppState>("load_app_state").catch((error) => {
          console.error("Failed to load app state:", error)
          return appStateRef.current
        }),
      ])
      const normalizedAppState = normalizeAppState(loadedAppState)
      appStateRef.current = normalizedAppState
      setAppState(normalizedAppState)
      const normalizedConfig = normalizeConfig(loadedConfig)
      launchBackgroundThrottling ??= normalizedConfig.background_throttling
      configRef.current = normalizedConfig
      setConfig(normalizedConfig)
    } catch (error) {
      console.error("Failed to load config:", error)
      launchBackgroundThrottling ??= defaultConfig.background_throttling
      configRef.current = defaultConfig
      setConfig(defaultConfig)
    } finally {
      setIsLoaded(true)
      markConfigReady()
      // Not awaited: the backend answers once startup has unlocked saved
      // passwords, which may wait on a credential store prompt.
      void refreshSecretStatus()
    }
  }, [refreshSecretStatus])

  const saveConfig = useCallback(async (newConfig: Partial<AppConfig>) => {
    const save = configSaveQueueRef.current.then(async () => {
      while (true) {
        const baseConfig = configRef.current
        const updatedConfig = normalizeConfig({ ...baseConfig, ...newConfig })
        try {
          await invoke("save_config", { config: updatedConfig })
        } catch (error) {
          console.error("Failed to save config:", error)
          throw error
        }

        if (configRef.current === baseConfig) {
          configRef.current = updatedConfig
          setConfig(updatedConfig)
          return
        }
      }
    })
    configSaveQueueRef.current = save.catch(() => undefined)
    return save
  }, [])

  const saveAppState = useCallback(async (update: Partial<AppState>) => {
    const save = appStateSaveQueueRef.current.then(async () => {
      const updatedState = normalizeAppState({ ...appStateRef.current, ...update })
      await invoke("save_app_state", { state: updatedState })
      appStateRef.current = updatedState
      setAppState(updatedState)
    })
    appStateSaveQueueRef.current = save.catch(() => undefined)
    return save
  }, [])

  const applySecretStatus = useCallback(
    (status: SecretBackendStatus) => {
      const normalized = normalizeSecretStatus(status)
      setSecretStatus(normalized)
      updateConfigState((prev) => ({ ...prev, secret_storage_mode: normalized.storageMode }))
      return normalized
    },
    [updateConfigState]
  )

  const setSecretStorageMode = useCallback(
    async (mode: SecretStorageMode, password?: string) => {
      const switchingToPassword =
        mode === "password" && configRef.current.secret_storage_mode !== "password"
      const status = applySecretStatus(
        await invoke<SecretBackendStatus>("set_secret_storage_mode", {
          input: { mode, password: password || null },
        })
      )
      if (switchingToPassword) {
        // The backend turns the startup prompt on with master password mode.
        updateConfigState((prev) => ({ ...prev, prompt_unlock_vault_on_startup: true }))
      }
      return status
    },
    [applySecretStatus, updateConfigState]
  )

  const unlockSecretVault = useCallback(
    async (password: string) =>
      applySecretStatus(
        await invoke<SecretBackendStatus>("unlock_secret_vault", { input: { password } })
      ),
    [applySecretStatus]
  )

  const lockSecretVault = useCallback(
    async () => applySecretStatus(await invoke<SecretBackendStatus>("lock_secret_vault")),
    [applySecretStatus]
  )

  const retrySystemUnlock = useCallback(
    async () => applySecretStatus(await invoke<SecretBackendStatus>("retry_system_unlock")),
    [applySecretStatus]
  )

  const resetSavedPasswords = useCallback(
    async () => applySecretStatus(await invoke<SecretBackendStatus>("reset_saved_passwords")),
    [applySecretStatus]
  )

  const changeVaultPassword = useCallback(
    async (currentPassword: string, newPassword: string) =>
      applySecretStatus(
        await invoke<SecretBackendStatus>("change_vault_password", {
          input: { currentPassword, newPassword },
        })
      ),
    [applySecretStatus]
  )

  const setMasterPassword = useCallback(
    async (password: string) =>
      applySecretStatus(
        await invoke<SecretBackendStatus>("set_master_password", { input: { password } })
      ),
    [applySecretStatus]
  )

  const removeMasterPassword = useCallback(
    async () => applySecretStatus(await invoke<SecretBackendStatus>("remove_master_password")),
    [applySecretStatus]
  )

  const listSavedSecrets = useCallback(
    async () => invoke<SavedSecretEntry[]>("list_saved_secrets"),
    []
  )

  const getSavedSecret = useCallback(
    async (key: string) => invoke<string>("get_saved_secret", { input: { key } }),
    []
  )

  const deleteSavedSecret = useCallback(
    async (key: string) => invoke<boolean>("delete_saved_secret", { input: { key } }),
    []
  )

  const updateTheme = useCallback(
    async (theme: string) => {
      await saveConfig({ theme })
    },
    [saveConfig]
  )

  const updateLanguage = useCallback(
    async (language: string) => {
      await saveConfig({ language })
    },
    [saveConfig]
  )

  useEffect(() => {
    loadConfig()
  }, [loadConfig])

  useEffect(() => onSyncApplied(["settings"], () => void loadConfig()), [loadConfig])

  const contextValue = useMemo<ConfigContextType>(
    () => ({
      config,
      appState,
      isLoaded,
      secretStatus,
      isSecretStatusLoaded,
      updateTheme,
      updateLanguage,
      saveConfig,
      saveAppState,
      loadConfig,
      refreshSecretStatus,
      setSecretStorageMode,
      unlockSecretVault,
      lockSecretVault,
      retrySystemUnlock,
      resetSavedPasswords,
      changeVaultPassword,
      setMasterPassword,
      removeMasterPassword,
      listSavedSecrets,
      getSavedSecret,
      deleteSavedSecret,
    }),
    [
      config,
      appState,
      isLoaded,
      secretStatus,
      isSecretStatusLoaded,
      updateTheme,
      updateLanguage,
      saveConfig,
      saveAppState,
      loadConfig,
      refreshSecretStatus,
      setSecretStorageMode,
      unlockSecretVault,
      lockSecretVault,
      retrySystemUnlock,
      resetSavedPasswords,
      changeVaultPassword,
      setMasterPassword,
      removeMasterPassword,
      listSavedSecrets,
      getSavedSecret,
      deleteSavedSecret,
    ]
  )

  return <ConfigContext.Provider value={contextValue}>{children}</ConfigContext.Provider>
}

export function useConfig() {
  const context = useContext(ConfigContext)
  if (context === undefined) {
    throw new Error("useConfig must be used within a ConfigProvider")
  }
  return context
}
