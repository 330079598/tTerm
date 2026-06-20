import React, { createContext, useContext, useEffect, useState, useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"

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
  cursor_style: "bar" | "block" | "underline"
  terminal_shell: "auto" | "cmd" | "powershell" | "pwsh" | "custom"
  terminal_shell_custom_path: string
  terminal_shell_custom_args: string
  secret_vault_enabled: boolean
  secret_storage_mode: SecretStorageMode
  prompt_unlock_vault_on_startup: boolean
  scrollback_lines: number
  terminal_padding_left_px: number
  terminal_padding_right_px: number
  terminal_padding_bottom_px: number
  startup_session_restore_mode: "active" | "all"
  show_jump_host_connection_info: boolean
  sftp_paste_upload_enabled: boolean
  monitor_refresh_interval_secs: number
  update_channel: "stable" | "beta-dev"
  auto_download_updates: boolean
  update_check_frequency: UpdateCheckFrequency
  last_update_check_at: number | null
  collapsed_profile_group_keys: string[]
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
  cursor_style: "block",
  terminal_shell: "auto",
  terminal_shell_custom_path: "",
  terminal_shell_custom_args: "",
  secret_vault_enabled: true,
  secret_storage_mode: "hybrid",
  prompt_unlock_vault_on_startup: false,
  scrollback_lines: 10000,
  terminal_padding_left_px: 6,
  terminal_padding_right_px: 0,
  terminal_padding_bottom_px: 0,
  startup_session_restore_mode: "active",
  show_jump_host_connection_info: true,
  sftp_paste_upload_enabled: false,
  monitor_refresh_interval_secs: 5,
  update_channel: defaultUpdateChannel,
  auto_download_updates: true,
  update_check_frequency: "daily",
  last_update_check_at: null,
  collapsed_profile_group_keys: [],
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
        : "hybrid",
    prompt_unlock_vault_on_startup: config.prompt_unlock_vault_on_startup === true,
    monitor_refresh_interval_secs: normalizeMonitorRefreshInterval(
      config.monitor_refresh_interval_secs
    ),
    update_channel: config.update_channel === "beta-dev" ? "beta-dev" : "stable",
    auto_download_updates: config.auto_download_updates !== false,
    terminal_padding_left_px: normalizeTerminalPadding(config.terminal_padding_left_px, 6),
    terminal_padding_right_px: normalizeTerminalPadding(config.terminal_padding_right_px, 0),
    terminal_padding_bottom_px: normalizeTerminalPadding(config.terminal_padding_bottom_px, 0),
    update_check_frequency: normalizeUpdateCheckFrequency(config.update_check_frequency),
    last_update_check_at:
      typeof config.last_update_check_at === "number" ? config.last_update_check_at : null,
    collapsed_profile_group_keys: collapsedProfileGroupKeys,
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
  const [secretStatus, setSecretStatus] = useState<SecretBackendStatus>(defaultSecretStatus)
  const [isLoaded, setIsLoaded] = useState(false)

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
      setConfig(normalizeConfig(loadedConfig))
      setSecretStatus(normalizeSecretStatus(loadedSecretStatus))
    } catch (error) {
      console.error("Failed to load config:", error)
      setConfig(defaultConfig)
      setSecretStatus(defaultSecretStatus)
    } finally {
      setIsLoaded(true)
      markConfigReady()
    }
  }, [])

  const saveConfig = useCallback(
    async (newConfig: Partial<AppConfig>) => {
      const updatedConfig = normalizeConfig({ ...config, ...newConfig })
      try {
        await invoke("save_config", { config: updatedConfig })
        setConfig(updatedConfig)
      } catch (error) {
        console.error("Failed to save config:", error)
        throw error
      }
    },
    [config]
  )

  const setSecretVaultEnabled = useCallback(async (enabled: boolean) => {
    const status = await invoke<SecretBackendStatus>("set_secret_vault_enabled", { enabled })
    const normalized = normalizeSecretStatus(status)
    setSecretStatus(normalized)
    setConfig((prev) => ({
      ...prev,
      secret_storage_mode: normalized.storageMode,
      secret_vault_enabled: normalized.vaultEnabled,
    }))
    return normalized
  }, [])

  const setSecretStorageMode = useCallback(async (mode: SecretStorageMode) => {
    const status = await invoke<SecretBackendStatus>("set_secret_storage_mode", {
      input: { mode },
    })
    const normalized = normalizeSecretStatus(status)
    setSecretStatus(normalized)
    setConfig((prev) => ({
      ...prev,
      secret_storage_mode: normalized.storageMode,
      secret_vault_enabled: normalized.vaultEnabled,
    }))
    return normalized
  }, [])

  const unlockSecretVault = useCallback(async (password: string, enableVault = false) => {
    const status = await invoke<SecretBackendStatus>("unlock_secret_vault", {
      input: { password, enableVault },
    })
    const normalized = normalizeSecretStatus(status)
    setSecretStatus(normalized)
    if (enableVault) {
      setConfig((prev) => ({ ...prev, secret_vault_enabled: true }))
    }
    return normalized
  }, [])

  const lockSecretVault = useCallback(async () => {
    const status = await invoke<SecretBackendStatus>("lock_secret_vault")
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
