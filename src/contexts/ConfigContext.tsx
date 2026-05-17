import React, { createContext, useContext, useEffect, useState, useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"

import { detectSystemLanguage } from "@/i18n/language"
import { markConfigReady } from "@/lib/startup"

export interface SecretBackendStatus {
  activeBackend: "system" | "vault" | "memory"
  keyringAvailable: boolean
  strongholdEnabled: boolean
  strongholdUnlocked: boolean
  persistenceAvailable: boolean
  message?: string | null
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
  scrollback_lines: number
  startup_session_restore_mode: "active" | "all"
  show_jump_host_connection_info: boolean
  update_channel: "stable" | "beta-dev"
  auto_download_updates: boolean
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
  secret_vault_enabled: false,
  scrollback_lines: 10000,
  startup_session_restore_mode: "active",
  show_jump_host_connection_info: true,
  update_channel: defaultUpdateChannel,
  auto_download_updates: true,
}

function normalizeConfig(config: Partial<AppConfig>): AppConfig {
  return {
    ...defaultConfig,
    ...config,
    startup_session_restore_mode: config.startup_session_restore_mode === "all" ? "all" : "active",
    show_jump_host_connection_info: config.show_jump_host_connection_info !== false,
    update_channel: config.update_channel === "beta-dev" ? "beta-dev" : "stable",
    auto_download_updates: config.auto_download_updates !== false,
  }
}

const defaultSecretStatus: SecretBackendStatus = {
  activeBackend: "memory",
  keyringAvailable: false,
  strongholdEnabled: false,
  strongholdUnlocked: false,
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
  unlockSecretVault: (password: string, enableVault?: boolean) => Promise<SecretBackendStatus>
  lockSecretVault: () => Promise<SecretBackendStatus>
}

const ConfigContext = createContext<ConfigContextType | undefined>(undefined)

function normalizeSecretStatus(status?: Partial<SecretBackendStatus>): SecretBackendStatus {
  return {
    activeBackend: (status?.activeBackend as SecretBackendStatus["activeBackend"]) ?? "memory",
    keyringAvailable: status?.keyringAvailable ?? false,
    strongholdEnabled: status?.strongholdEnabled ?? false,
    strongholdUnlocked: status?.strongholdUnlocked ?? false,
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
    setConfig((prev) => ({ ...prev, secret_vault_enabled: enabled }))
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
        unlockSecretVault,
        lockSecretVault,
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
