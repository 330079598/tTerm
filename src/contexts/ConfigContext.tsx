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
  message?: string | null
}

/** Where the key that unlocks saved passwords comes from. */
export type SecretStorageMode = "system" | "password" | "memory"
export type TabWidthMode = "adaptive" | "standard"
export type TerminalLogFormat = "raw" | "plain" | "both"
export type TerminalRenderer = "webgl" | "canvas"
export type MonitorMetricId =
  | "cpu"
  | "memory"
  | "network"
  | "ip"
  | "latency"
  | "disk"
  | "load"
  | "uptime"

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
  return getDetectedPlatform() === "macos" ? "canvas" : "webgl"
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
  last_update_check_at: number | null
  collapsed_profile_group_keys: string[]
  tab_width_mode: TabWidthMode
  tab_standard_width: number
  terminal_log_enabled: boolean
  terminal_log_directory: string
  terminal_log_format: TerminalLogFormat
  terminal_log_name_template: string
  terminal_log_max_file_size_mb: number
  terminal_log_compress: boolean
  sftp_transfer_parallelism: number
  reconnect_enabled: boolean
  reconnect_max_attempts: number
  zmodem_auto_detect_enabled: boolean
  zmodem_download_directory: string
  /** Extra sudo password prompt regexes; an optional `user` group names the account. */
  sudo_prompt_patterns: string[]
  keymap: KeymapConfig
}

const defaultUpdateChannel = /-(alpha|beta|rc|dev)(\.|$)/.test(
  import.meta.env.PACKAGE_VERSION ?? ""
)
  ? "beta-dev"
  : "stable"

export const DEFAULT_TERMINAL_FONT_FAMILY =
  '"JetBrains Mono Nerd Font", "JetBrainsMono Nerd Font", "JetBrains Mono", "Fira Code", Menlo, Monaco, monospace'

const defaultConfig: AppConfig = {
  theme: "default",
  language: detectSystemLanguage(),
  font_family: DEFAULT_TERMINAL_FONT_FAMILY,
  font_size: 14,
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
  last_update_check_at: null,
  collapsed_profile_group_keys: [],
  tab_width_mode: "adaptive",
  tab_standard_width: 120,
  terminal_log_enabled: false,
  terminal_log_directory: "",
  terminal_log_format: "both",
  terminal_log_name_template: "{profile}-{host}-{yyyyMMdd-HHmmss}-{sessionId}",
  terminal_log_max_file_size_mb: 50,
  terminal_log_compress: false,
  sftp_transfer_parallelism: 4,
  reconnect_enabled: true,
  reconnect_max_attempts: 5,
  zmodem_auto_detect_enabled: true,
  zmodem_download_directory: "",
  sudo_prompt_patterns: [],
  keymap: { ...DEFAULT_KEYMAP_CONFIG, bindings: {} },
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

function normalizeConfig(config: Partial<AppConfig>): AppConfig {
  const collapsedProfileGroupKeys = Array.isArray(config.collapsed_profile_group_keys)
    ? config.collapsed_profile_group_keys.filter((item): item is string => typeof item === "string")
    : []

  return {
    ...defaultConfig,
    ...config,
    startup_session_restore_mode: config.startup_session_restore_mode === "all" ? "all" : "active",
    show_jump_host_connection_info: config.show_jump_host_connection_info !== false,
    sftp_paste_upload_enabled: config.sftp_paste_upload_enabled === true,
    secret_storage_mode: normalizeSecretStorageMode(config.secret_storage_mode),
    prompt_unlock_vault_on_startup: config.prompt_unlock_vault_on_startup === true,
    terminal_renderer:
      config.terminal_renderer === "canvas" || config.terminal_renderer === "webgl"
        ? config.terminal_renderer
        : getDefaultTerminalRenderer(),
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
    last_update_check_at:
      typeof config.last_update_check_at === "number" ? config.last_update_check_at : null,
    collapsed_profile_group_keys: collapsedProfileGroupKeys,
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
    keymap: normalizeKeymap(config.keymap),
  }
}

const defaultSecretStatus: SecretBackendStatus = {
  storageMode: "system",
  keyringAvailable: false,
  unlocked: false,
  hasMasterPassword: false,
  persistenceAvailable: false,
  migrationPending: false,
  message: null,
}

interface ConfigContextType {
  config: AppConfig
  isLoaded: boolean
  secretStatus: SecretBackendStatus
  updateTheme: (theme: string) => Promise<void>
  updateLanguage: (language: string) => Promise<void>
  saveConfig: (newConfig: Partial<AppConfig>) => Promise<void>
  loadConfig: () => Promise<void>
  refreshSecretStatus: () => Promise<SecretBackendStatus>
  setSecretStorageMode: (mode: SecretStorageMode, password?: string) => Promise<SecretBackendStatus>
  unlockSecretVault: (password: string) => Promise<SecretBackendStatus>
  lockSecretVault: () => Promise<SecretBackendStatus>
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
    message: status?.message ?? null,
  }
}

export function ConfigProvider({ children }: { children: React.ReactNode }) {
  const [config, setConfig] = useState<AppConfig>(defaultConfig)
  const configRef = useRef(config)
  const configSaveQueueRef = useRef<Promise<void>>(Promise.resolve())
  const [secretStatus, setSecretStatus] = useState<SecretBackendStatus>(defaultSecretStatus)
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
    }
  }, [])

  const loadConfig = useCallback(async (): Promise<void> => {
    try {
      const [loadedConfig, loadedSecretStatus] = await Promise.all([
        invoke<AppConfig>("load_config"),
        invoke<SecretBackendStatus>("get_secret_backend_status"),
      ])
      const normalizedConfig = normalizeConfig(loadedConfig)
      configRef.current = normalizedConfig
      setConfig(normalizedConfig)
      setSecretStatus(normalizeSecretStatus(loadedSecretStatus))
    } catch (error) {
      console.error("Failed to load config:", error)
      configRef.current = defaultConfig
      setConfig(defaultConfig)
      setSecretStatus(defaultSecretStatus)
    } finally {
      setIsLoaded(true)
      markConfigReady()
    }
  }, [])

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

  const contextValue = useMemo<ConfigContextType>(
    () => ({
      config,
      isLoaded,
      secretStatus,
      updateTheme,
      updateLanguage,
      saveConfig,
      loadConfig,
      refreshSecretStatus,
      setSecretStorageMode,
      unlockSecretVault,
      lockSecretVault,
      changeVaultPassword,
      setMasterPassword,
      removeMasterPassword,
      listSavedSecrets,
      getSavedSecret,
      deleteSavedSecret,
    }),
    [
      config,
      isLoaded,
      secretStatus,
      updateTheme,
      updateLanguage,
      saveConfig,
      loadConfig,
      refreshSecretStatus,
      setSecretStorageMode,
      unlockSecretVault,
      lockSecretVault,
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
