import { detectSystemLanguage } from "@/i18n/language"
import type { VerificationMethod } from "@/lib/userVerification"
import type { UpdateCheckFrequency } from "@/lib/updater"
import { DEFAULT_KEYMAP_CONFIG, type KeymapConfig } from "@/lib/keymap/keymap"
import {
  defaultKeywordHighlightConfig,
  type KeywordHighlightConfig,
} from "@/lib/keywordHighlight/config"
import { DEFAULT_MONITOR_VISIBLE_METRICS, type MonitorMetricId } from "@/lib/monitorMetrics"
import type { SftpStartDirectory } from "@/lib/sftpStartDirectory"
import { defaultTerminalFontFamily } from "@/lib/terminalFont"
import { DEFAULT_TAB_SETTINGS, type TabSettings } from "@/lib/tabSettings"
import { getDetectedPlatform } from "@/lib/uiPlatform"

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
export type TerminalLogFormat = "raw" | "plain" | "both"
export type TerminalRenderer = "webgl" | "canvas"
/** What WebKit does with the page while the window is hidden (macOS 14+). */
export type BackgroundThrottling = "throttle" | "disabled" | "suspend"
/** What closing the main window does; "tray" keeps tTerm running in the background. */
export type CloseBehavior = "ask" | "tray" | "quit"
export type WindowBlurMaterial = "acrylic" | "mica"
/** How the window lays out its surfaces; "islands" floats each pane as a rounded card. */
export type UiStyle = "classic" | "islands"
/** How the connections panel lists profiles: by group, or in one list by when they were last used. */
export type ProfileSort = "default" | "recent"
/** What the terminal bell does: flash the terminal, play a tone, or nothing. */
export type BellStyle = "visual" | "sound" | "none"

/** The sounds a notification can make: the system's, a tone tTerm plays, or a file. */
export const NOTIFICATION_SOUND_NAMES = [
  "system",
  "chime",
  "ding",
  "pop",
  "rise",
  "custom",
] as const
export type NotificationSoundName = (typeof NOTIFICATION_SOUND_NAMES)[number]

export function getDefaultTerminalRenderer(): TerminalRenderer {
  return "webgl"
}

/** How long a hidden terminal keeps its renderer; 0 keeps it for good. */
export const HIDDEN_RENDERER_RELEASE_SECS_OPTIONS = [0, 10, 30, 60, 300] as const
export const DEFAULT_HIDDEN_RENDERER_RELEASE_SECS = 10

export interface SavedSecretEntry {
  key: string
  profileId: string
  profileName: string
  label: string
  kind: string
}

export interface AppConfig extends TabSettings {
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
  /** Whether a restart shows each terminal tab's content from before. */
  restore_terminal_content: boolean
  show_jump_host_connection_info: boolean
  sftp_paste_upload_enabled: boolean
  /** Where SFTP drawers open; a connection can choose otherwise. */
  sftp_start_directory: SftpStartDirectory
  monitor_refresh_interval_secs: number
  monitor_visible_metrics: MonitorMetricId[]
  update_channel: "stable" | "beta-dev"
  auto_download_updates: boolean
  update_check_frequency: UpdateCheckFrequency
  terminal_log_enabled: boolean
  terminal_log_directory: string
  terminal_log_format: TerminalLogFormat
  terminal_log_name_template: string
  terminal_log_max_file_size_mb: number
  terminal_log_compress: boolean
  /** Start each line of the plain-text log with when it appeared. */
  terminal_log_plain_timestamps: boolean
  /** Also log what is typed, including passwords at prompts that do not echo. */
  terminal_log_record_input: boolean
  /** Remove logs last written more than this many days ago; 0 keeps them. */
  terminal_log_retention_days: number
  /** Remove the oldest logs while all of them take more than this many MiB; 0 is no limit. */
  terminal_log_max_total_mb: number
  sftp_transfer_parallelism: number
  /** SFTP upload bandwidth cap in KiB/s shared by all transfers; 0 = unlimited. */
  sftp_upload_limit_kib: number
  /** SFTP download bandwidth cap in KiB/s shared by all transfers; 0 = unlimited. */
  sftp_download_limit_kib: number
  reconnect_enabled: boolean
  reconnect_max_attempts: number
  zmodem_auto_detect_enabled: boolean
  zmodem_download_directory: string
  /** Proxy server that hosts left on the default connect through; empty for none. */
  default_proxy_id: string
  /** Extra sudo password prompt regexes; an optional `user` group names the account. */
  sudo_prompt_patterns: string[]
  /** A mark per character typed at a password prompt; off, only a key shows there. */
  password_feedback: boolean
  /** macOS: Option+key sends Meta (ESC prefix) instead of the alternate character. */
  mac_option_is_meta: boolean
  /** Ask before pasting multiple lines into a shell without bracketed paste mode. */
  confirm_multiline_paste: boolean
  /** Copy the terminal selection to the clipboard when the mouse button is released. */
  copy_on_select: boolean
  /** Right-click pastes instead of opening the menu; Shift+right-click still opens it. */
  right_click_paste: boolean
  /** Programs may copy to the clipboard with OSC 52; they may never read it. */
  osc52_clipboard_write: boolean
  /** Local shells mark their prompts and commands (OSC 133) for jumping and copying output. */
  command_marks: boolean
  /** Words and patterns colored in terminal output, in named sets. */
  keyword_highlight: KeywordHighlightConfig
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
  ui_style: UiStyle
  profile_sort: ProfileSort
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
  /** System notifications make a sound. */
  notify_sound: boolean
  notify_sound_name: NotificationSoundName
  /** The name of the file the "custom" sound was copied from; tTerm plays its own copy. */
  notify_sound_file: string
  bell_style: BellStyle
}

// The backend's `default_update_channel` makes the same choice.
const defaultUpdateChannel = /-(alpha|beta|rc|dev)(\.|$)/.test(
  import.meta.env.PACKAGE_VERSION ?? ""
)
  ? "beta-dev"
  : "stable"

export const DEFAULT_TERMINAL_FONT_SIZE = 14

export const DEFAULT_TERMINAL_FONT_FAMILY = defaultTerminalFontFamily()

export const defaultConfig: AppConfig = {
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
  terminal_padding_right_px: 6,
  terminal_padding_bottom_px: 4,
  startup_session_restore_mode: "active",
  restore_terminal_content: true,
  show_jump_host_connection_info: true,
  sftp_paste_upload_enabled: false,
  sftp_start_directory: "last",
  monitor_refresh_interval_secs: 5,
  monitor_visible_metrics: DEFAULT_MONITOR_VISIBLE_METRICS,
  update_channel: defaultUpdateChannel,
  auto_download_updates: true,
  update_check_frequency: "daily",
  ...DEFAULT_TAB_SETTINGS,
  terminal_log_enabled: false,
  terminal_log_directory: "",
  terminal_log_format: "both",
  terminal_log_name_template: "{year}-{month}-{day}/{profile}-{host}-{time}-{sessionId}",
  terminal_log_max_file_size_mb: 50,
  terminal_log_compress: false,
  terminal_log_plain_timestamps: true,
  terminal_log_record_input: false,
  terminal_log_retention_days: 0,
  terminal_log_max_total_mb: 0,
  sftp_transfer_parallelism: 4,
  sftp_upload_limit_kib: 0,
  sftp_download_limit_kib: 0,
  reconnect_enabled: true,
  reconnect_max_attempts: 5,
  zmodem_auto_detect_enabled: true,
  zmodem_download_directory: "",
  default_proxy_id: "",
  sudo_prompt_patterns: [],
  password_feedback: true,
  mac_option_is_meta: false,
  confirm_multiline_paste: true,
  copy_on_select: false,
  // Windows users expect it from PuTTY, Xshell and Windows Terminal.
  right_click_paste: getDetectedPlatform() === "windows",
  osc52_clipboard_write: true,
  command_marks: true,
  keyword_highlight: defaultKeywordHighlightConfig(),
  local_shell_integration: false,
  terminal_line_height: 1,
  terminal_letter_spacing: 0,
  keymap: { ...DEFAULT_KEYMAP_CONFIG, bindings: {} },
  background_throttling: "throttle",
  window_blur: false,
  window_blur_radius: 20,
  window_opacity_percent: 70,
  window_blur_material: "acrylic",
  ui_style: "classic",
  profile_sort: "default",
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
  notify_sound_name: "system",
  notify_sound_file: "",
  bell_style: "none",
}

/** Longest log retention the settings accept, in days (matches the backend). */
export const TERMINAL_LOG_MAX_RETENTION_DAYS = 3650
/** Largest log directory size limit the settings accept, in MiB (matches the backend). */
export const TERMINAL_LOG_MAX_TOTAL_MB = 1024 * 1024

export const MAX_BANDWIDTH_LIMIT_KIB = 10 * 1024 * 1024

export const RECONNECT_MAX_ATTEMPTS_MIN = 1
export const RECONNECT_MAX_ATTEMPTS_MAX = 99

export const TERMINAL_FONT_SIZE_RANGE = { min: 6, max: 72 } as const
export const TERMINAL_LINE_HEIGHT_RANGE = { min: 1, max: 2 } as const
export const TERMINAL_LETTER_SPACING_RANGE = { min: -5, max: 10 } as const

export const WINDOW_OPACITY_PERCENT_RANGE = { min: 30, max: 95 } as const
export const WINDOW_BLUR_RADIUS_RANGE = { min: 1, max: 60 } as const
export const NOTIFY_COMMAND_MIN_SECS_RANGE = { min: 1, max: 3600 } as const
export const TOAST_MAX_VISIBLE_RANGE = { min: 1, max: 5 } as const

/** State the app keeps for itself rather than settings the user chose. */
export interface AppState {
  lastUpdateCheckAt: number | null
  collapsedProfileGroupKeys: string[]
}

export const defaultAppState: AppState = {
  lastUpdateCheckAt: null,
  collapsedProfileGroupKeys: [],
}

export const defaultSecretStatus: SecretBackendStatus = {
  storageMode: "system",
  keyringAvailable: false,
  unlocked: false,
  hasMasterPassword: false,
  persistenceAvailable: false,
  migrationPending: false,
  verificationMethod: "none",
  message: null,
}
