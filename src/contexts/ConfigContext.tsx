import React, { createContext, useContext, useEffect, useRef, useState, useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"
import { platform } from "@tauri-apps/plugin-os"

import { detectSystemLanguage } from "@/i18n/language"
import { markConfigReady } from "@/lib/startup"
import type { UpdateCheckFrequency } from "@/lib/updater"

export interface SecretBackendStatus {
  activeBackend: "system" | "vault" | "memory"
  storageMode: SecretStorageMode
  keyringAvailable: boolean
  vaultEnabled: boolean
  vaultUnlocked: boolean
  persistenceAvailable: boolean
  message?: string | null
}

export type SecretStorageMode = "auto" | "system" | "vault" | "hybrid" | "memory"
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

function getDefaultSecretStorageMode(): SecretStorageMode {
  try {
    _cachedPlatform ??= platform()
    return _cachedPlatform === "windows" ? "system" : "hybrid"
  } catch {
    return "hybrid"
  }
}

export interface CopySecretStoreResult {
  copied: number
  skipped: number
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
}

const defaultUpdateChannel = /-(alpha|beta|rc|dev)(\.|$)/.test(
  import.meta.env.PACKAGE_VERSION ?? ""
)
  ? "beta-dev"
  : "stable"

const defaultConfig: AppConfig = {
  theme: "default",
  language: detectSystemLanguage(),
  font_family:
    '"JetBrains Mono Nerd Font", "JetBrainsMono Nerd Font", "JetBrains Mono", "Fira Code", Menlo, Monaco, monospace',
  font_size: 14,
  ui_scale_percent: 100,
  cursor_style: "block",
  terminal_shell: "auto",
  terminal_shell_custom_path: "",
  terminal_shell_custom_args: "",
  secret_vault_enabled: true,
  secret_storage_mode: getDefaultSecretStorageMode(),
  prompt_unlock_vault_on_startup: false,
  scrollback_lines: 10000,
  terminal_renderer: "webgl",
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
    secret_storage_mode:
      config.secret_storage_mode === "system" ||
      config.secret_storage_mode === "vault" ||
      config.secret_storage_mode === "hybrid" ||
      config.secret_storage_mode === "memory"
        ? config.secret_storage_mode
        : getDefaultSecretStorageMode(),
    prompt_unlock_vault_on_startup: config.prompt_unlock_vault_on_startup === true,
    terminal_renderer: config.terminal_renderer === "canvas" ? "canvas" : "webgl",
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
    ui_scale_percent: normalizeUiScalePercent(config.ui_scale_percent),
  }
}

const defaultSecretStatus: SecretBackendStatus = {
  activeBackend: "memory",
  storageMode: "auto",
  keyringAvailable: false,
  vaultEnabled: false,
  vaultUnlocked: false,
  persistenceAvailable: false,
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
  setSecretVaultEnabled: (enabled: boolean) => Promise<SecretBackendStatus>
  setSecretStorageMode: (mode: SecretStorageMode) => Promise<SecretBackendStatus>
  unlockSecretVault: (password: string, enableVault?: boolean) => Promise<SecretBackendStatus>
  lockSecretVault: () => Promise<SecretBackendStatus>
  changeVaultPassword: (
    currentPassword: string,
    newPassword: string
  ) => Promise<SecretBackendStatus>
  copySecretStore: (direction: "systemToVault" | "vaultToSystem") => Promise<CopySecretStoreResult>
  listSavedSecrets: () => Promise<SavedSecretEntry[]>
  deleteSavedSecret: (key: string) => Promise<boolean>
}

const ConfigContext = createContext<ConfigContextType | undefined>(undefined)

function normalizeSecretStatus(status?: Partial<SecretBackendStatus>): SecretBackendStatus {
  return {
    activeBackend: (status?.activeBackend as SecretBackendStatus["activeBackend"]) ?? "memory",
    storageMode: (status?.storageMode as SecretStorageMode) ?? "auto",
    keyringAvailable: status?.keyringAvailable ?? false,
    vaultEnabled: status?.vaultEnabled ?? false,
    vaultUnlocked: status?.vaultUnlocked ?? false,
    persistenceAvailable: status?.persistenceAvailable ?? false,
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

  const setSecretVaultEnabled = useCallback(
    async (enabled: boolean) => {
      const status = await invoke<SecretBackendStatus>("set_secret_vault_enabled", { enabled })
      const normalized = normalizeSecretStatus(status)
      setSecretStatus(normalized)
      updateConfigState((prev) => ({
        ...prev,
        secret_storage_mode: normalized.storageMode,
        secret_vault_enabled: normalized.vaultEnabled,
      }))
      return normalized
    },
    [updateConfigState]
  )

  const setSecretStorageMode = useCallback(
    async (mode: SecretStorageMode) => {
      const status = await invoke<SecretBackendStatus>("set_secret_storage_mode", {
        input: { mode },
      })
      const normalized = normalizeSecretStatus(status)
      setSecretStatus(normalized)
      updateConfigState((prev) => ({
        ...prev,
        secret_storage_mode: normalized.storageMode,
        secret_vault_enabled: normalized.vaultEnabled,
      }))
      return normalized
    },
    [updateConfigState]
  )

  const unlockSecretVault = useCallback(
    async (password: string, enableVault = false) => {
      const status = await invoke<SecretBackendStatus>("unlock_secret_vault", {
        input: { password, enableVault },
      })
      const normalized = normalizeSecretStatus(status)
      setSecretStatus(normalized)
      if (enableVault) {
        updateConfigState((prev) => ({ ...prev, secret_vault_enabled: true }))
      }
      return normalized
    },
    [updateConfigState]
  )

  const lockSecretVault = useCallback(async () => {
    const status = await invoke<SecretBackendStatus>("lock_secret_vault")
    const normalized = normalizeSecretStatus(status)
    setSecretStatus(normalized)
    return normalized
  }, [])

  const changeVaultPassword = useCallback(async (currentPassword: string, newPassword: string) => {
    const status = await invoke<SecretBackendStatus>("change_vault_password", {
      input: { currentPassword, newPassword },
    })
    const normalized = normalizeSecretStatus(status)
    setSecretStatus(normalized)
    return normalized
  }, [])

  const copySecretStore = useCallback(
    async (direction: "systemToVault" | "vaultToSystem") =>
      invoke<CopySecretStoreResult>("copy_secret_store", { input: { direction } }),
    []
  )

  const listSavedSecrets = useCallback(
    async () => invoke<SavedSecretEntry[]>("list_saved_secrets"),
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

  return (
    <ConfigContext.Provider
      value={{
        config,
        isLoaded,
        secretStatus,
        updateTheme,
        updateLanguage,
        saveConfig,
        loadConfig,
        refreshSecretStatus,
        setSecretVaultEnabled,
        setSecretStorageMode,
        unlockSecretVault,
        lockSecretVault,
        changeVaultPassword,
        copySecretStore,
        listSavedSecrets,
        deleteSavedSecret,
      }}
    >
      {children}
    </ConfigContext.Provider>
  )
}

export function useConfig() {
  const context = useContext(ConfigContext)
  if (context === undefined) {
    throw new Error("useConfig must be used within a ConfigProvider")
  }
  return context
}
