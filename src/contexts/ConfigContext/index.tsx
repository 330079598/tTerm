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

import {
  type SecretBackendStatus,
  type SecretStorageMode,
  type BackgroundThrottling,
  type SavedSecretEntry,
  type AppConfig,
  defaultConfig,
  type AppState,
  defaultAppState,
} from "@/lib/appConfig"
import { normalizeConfig, normalizeAppState } from "@/lib/appConfigNormalize"
import { markConfigReady } from "@/lib/startup"
import { onSyncApplied } from "@/lib/sync"
import { useSecretStore } from "@/contexts/ConfigContext/useSecretStore"
import { useWindowAppearance } from "@/contexts/ConfigContext/useWindowAppearance"

let launchBackgroundThrottling: BackgroundThrottling | null = null

/**
 * The policy the window was created with. The backend reads it only when it
 * builds the window, so a saved value that differs takes a restart.
 */
export function getLaunchBackgroundThrottling(): BackgroundThrottling | null {
  return launchBackgroundThrottling
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
export function ConfigProvider({ children }: { children: React.ReactNode }) {
  const [config, setConfig] = useState<AppConfig>(defaultConfig)
  const configRef = useRef(config)
  const configSaveQueueRef = useRef<Promise<void>>(Promise.resolve())
  const [appState, setAppState] = useState<AppState>(defaultAppState)
  const appStateRef = useRef(appState)
  const appStateSaveQueueRef = useRef<Promise<void>>(Promise.resolve())
  const [isLoaded, setIsLoaded] = useState(false)

  const updateConfigState = useCallback((update: (current: AppConfig) => AppConfig) => {
    const updatedConfig = update(configRef.current)
    configRef.current = updatedConfig
    setConfig(updatedConfig)
  }, [])

  useWindowAppearance(config, isLoaded)

  const {
    secretStatus,
    isSecretStatusLoaded,
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
  } = useSecretStore({ configRef, updateConfigState })

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
