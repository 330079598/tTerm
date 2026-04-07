import React, { createContext, useCallback, useContext, useEffect, useState } from "react"
import { useConfig } from "@/contexts/ConfigContext"
import type { CustomTheme, PresetTheme, PresetThemeId, Theme } from "@/types/theme"
import { applyCustomTheme } from "@/lib/themeUtils"

const PRESET_THEMES_DATA: PresetTheme[] = [
  { id: "default", name: "Default", description: "Tabby inspired dark theme", isCustom: false },
  { id: "light", name: "Light", description: "Clean light theme", isCustom: false },
  { id: "ocean", name: "Ocean", description: "Deep blue ocean theme", isCustom: false },
  { id: "forest", name: "Forest", description: "Natural green forest theme", isCustom: false },
  { id: "sunset", name: "Sunset", description: "Warm orange sunset theme", isCustom: false },
  { id: "ubuntu", name: "Ubuntu", description: "Ubuntu inspired theme", isCustom: false },
]

const STORAGE_KEY = "custom-themes"

interface ThemeContextType {
  currentTheme: string
  availableThemes: Theme[]
  presetThemes: PresetTheme[]
  customThemes: CustomTheme[]
  setTheme: (themeId: string) => Promise<void>
  createCustomTheme: (theme: Omit<CustomTheme, "id">) => Promise<CustomTheme>
  updateCustomTheme: (id: string, updates: Partial<CustomTheme>) => Promise<void>
  deleteCustomTheme: (id: string) => Promise<void>
  duplicateTheme: (themeId: string, newName: string) => Promise<CustomTheme>
  getTheme: (id: string) => Theme | undefined
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined)

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const { config, updateTheme, isLoaded } = useConfig()
  const [customThemes, setCustomThemes] = useState<CustomTheme[]>([])

  // 从 localStorage 加载自定义主题
  useEffect(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY)
      if (stored) {
        const themes = JSON.parse(stored) as CustomTheme[]
        setCustomThemes(themes)
      }
    } catch (error) {
      console.error("Failed to load custom themes:", error)
    }
  }, [])

  // 保存自定义主题到 localStorage
  const saveCustomThemes = useCallback((themes: CustomTheme[]) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(themes))
      setCustomThemes(themes)
    } catch (error) {
      console.error("Failed to save custom themes:", error)
      throw error
    }
  }, [])

  // 应用主题
  useEffect(() => {
    if (isLoaded) {
      const themeId = config.theme || "default"
      const customTheme = customThemes.find((t) => t.id === themeId)

      if (customTheme) {
        // 应用自定义主题
        document.documentElement.removeAttribute("data-theme")
        applyCustomTheme(customTheme.colors)
      } else {
        // 应用预设主题
        document.documentElement.setAttribute("data-theme", themeId)
      }
    }
  }, [config.theme, isLoaded, customThemes])

  const setTheme = async (themeId: string) => {
    const customTheme = customThemes.find((t) => t.id === themeId)

    if (customTheme) {
      document.documentElement.removeAttribute("data-theme")
      applyCustomTheme(customTheme.colors)
    } else {
      document.documentElement.setAttribute("data-theme", themeId)
    }

    await updateTheme(themeId)
  }

  const createCustomTheme = async (theme: Omit<CustomTheme, "id">): Promise<CustomTheme> => {
    const newTheme: CustomTheme = {
      ...theme,
      id: `custom-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
    }

    const updatedThemes = [...customThemes, newTheme]
    saveCustomThemes(updatedThemes)

    return newTheme
  }

  const updateCustomTheme = async (id: string, updates: Partial<CustomTheme>) => {
    const updatedThemes = customThemes.map((theme) =>
      theme.id === id ? { ...theme, ...updates, updatedAt: Date.now() } : theme
    )

    saveCustomThemes(updatedThemes)

    // 如果更新的是当前主题，重新应用
    if (config.theme === id) {
      const updatedTheme = updatedThemes.find((t) => t.id === id)
      if (updatedTheme) {
        applyCustomTheme(updatedTheme.colors)
      }
    }
  }

  const deleteCustomTheme = async (id: string) => {
    const updatedThemes = customThemes.filter((theme) => theme.id !== id)
    saveCustomThemes(updatedThemes)

    // 如果删除的是当前主题，切换到默认主题
    if (config.theme === id) {
      await setTheme("default")
    }
  }

  const duplicateTheme = async (themeId: string, newName: string): Promise<CustomTheme> => {
    const sourceTheme = customThemes.find((t) => t.id === themeId)

    if (sourceTheme) {
      // 复制自定义主题
      return createCustomTheme({
        name: newName,
        description: sourceTheme.description,
        colors: { ...sourceTheme.colors },
        baseTheme: sourceTheme.baseTheme,
        isCustom: true,
        createdAt: Date.now(),
        updatedAt: Date.now(),
      })
    } else {
      // 从预设主题创建
      const { createCustomThemeFromPreset } = await import("@/lib/themeUtils")
      const themeData = createCustomThemeFromPreset(
        themeId as PresetThemeId,
        newName,
        `Based on ${themeId}`
      )
      return createCustomTheme(themeData)
    }
  }

  const getTheme = (id: string): Theme | undefined => {
    return PRESET_THEMES_DATA.find((t) => t.id === id) || customThemes.find((t) => t.id === id)
  }

  const availableThemes: Theme[] = [...PRESET_THEMES_DATA, ...customThemes]

  return (
    <ThemeContext.Provider
      value={{
        currentTheme: config.theme || "default",
        availableThemes,
        presetThemes: PRESET_THEMES_DATA,
        customThemes,
        setTheme,
        createCustomTheme,
        updateCustomTheme,
        deleteCustomTheme,
        duplicateTheme,
        getTheme,
      }}
    >
      {children}
    </ThemeContext.Provider>
  )
}

export function useTheme() {
  const context = useContext(ThemeContext)
  if (context === undefined) {
    throw new Error("useTheme must be used within a ThemeProvider")
  }
  return context
}

// 导出类型和常量
export type { Theme, CustomTheme, PresetTheme, PresetThemeId } from "@/types/theme"
export const PRESET_THEMES = PRESET_THEMES_DATA
