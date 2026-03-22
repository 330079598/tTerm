import React, { createContext, useContext, useEffect, useState, useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"

export interface AppConfig {
  theme: string
  language: string
  font_family: string
  font_size: number
}

const defaultConfig: AppConfig = {
  theme: "default",
  language: "en",
  font_family: '"JetBrainsMono Nerd Font", "JetBrains Mono", "Fira Code", Menlo, Monaco, monospace',
  font_size: 14,
}

interface ConfigContextType {
  config: AppConfig
  isLoaded: boolean
  updateTheme: (theme: string) => Promise<void>
  updateLanguage: (language: string) => Promise<void>
  saveConfig: (newConfig: Partial<AppConfig>) => Promise<void>
  loadConfig: () => Promise<void>
}

const ConfigContext = createContext<ConfigContextType | undefined>(undefined)

export function ConfigProvider({ children }: { children: React.ReactNode }) {
  const [config, setConfig] = useState<AppConfig>(defaultConfig)
  const [isLoaded, setIsLoaded] = useState(false)

  const loadConfig = useCallback(async () => {
    try {
      const loadedConfig = await invoke<AppConfig>("load_config")
      setConfig(loadedConfig)
    } catch (error) {
      console.error("Failed to load config:", error)
      setConfig(defaultConfig)
    } finally {
      setIsLoaded(true)
    }
  }, [])

  const saveConfig = useCallback(
    async (newConfig: Partial<AppConfig>) => {
      const updatedConfig = { ...config, ...newConfig }
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

  // Load config on mount
  useEffect(() => {
    loadConfig()
  }, [loadConfig])

  return (
    <ConfigContext.Provider
      value={{
        config,
        isLoaded,
        updateTheme,
        updateLanguage,
        saveConfig,
        loadConfig,
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
