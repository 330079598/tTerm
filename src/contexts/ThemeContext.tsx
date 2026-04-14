import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
} from "react"

import { useConfig } from "@/contexts/ConfigContext"
import { announceThemeReady } from "@/lib/startup"
import { applyThemeToDom, cacheTheme, resolveThemeCache } from "@/lib/themePreloader"
import type { CustomTheme, PresetTheme, PresetThemeId, Theme } from "@/types/theme"

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
  const [themesLoaded, setThemesLoaded] = useState(false)

  const applyAndCacheTheme = useCallback((themeId: string, themes: CustomTheme[]) => {
    const themeCache = resolveThemeCache(themeId, themes)
    applyThemeToDom(themeCache)
    cacheTheme(themeCache)
    return themeCache.id
  }, [])

  // Load custom themes from localStorage on mount
  useEffect(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY)
      if (stored) {
        const themes = JSON.parse(stored) as CustomTheme[]
        setCustomThemes(themes)
      }
    } catch (error) {
      console.error("Failed to load custom themes:", error)
    } finally {
      setThemesLoaded(true)
    }
  }, [])

  // Save custom themes to localStorage
  const saveCustomThemes = useCallback((themes: CustomTheme[]) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(themes))
      setCustomThemes(themes)
    } catch (error) {
      console.error("Failed to save custom themes:", error)
      throw error
    }
  }, [])

  // Calibrate theme with Tauri config after loading
  // This is the key to dual-source strategy:
  // - localStorage may be cleared, but Tauri config is reliable
  // - On startup: use cached theme first (fast), then calibrate with real config
  useLayoutEffect(() => {
    if (!isLoaded || !themesLoaded) return

    const resolvedThemeId = applyAndCacheTheme(config.theme || "default", customThemes)

    if (resolvedThemeId !== (config.theme || "default")) {
      void updateTheme(resolvedThemeId)
    }

    announceThemeReady()
  }, [applyAndCacheTheme, config.theme, customThemes, isLoaded, themesLoaded, updateTheme])

  const setTheme = async (themeId: string): Promise<void> => {
    const resolvedThemeId = applyAndCacheTheme(themeId, customThemes)
    await updateTheme(resolvedThemeId)
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

    if (config.theme === id) {
      applyAndCacheTheme(id, updatedThemes)
    }
  }

  const deleteCustomTheme = async (id: string) => {
    const updatedThemes = customThemes.filter((theme) => theme.id !== id)
    saveCustomThemes(updatedThemes)

    if (config.theme === id) {
      await setTheme("default")
    }
  }

  const duplicateTheme = async (themeId: string, newName: string): Promise<CustomTheme> => {
    const sourceTheme = customThemes.find((t) => t.id === themeId)

    if (sourceTheme) {
      return createCustomTheme({
        name: newName,
        description: sourceTheme.description,
        colors: { ...sourceTheme.colors },
        baseTheme: sourceTheme.baseTheme,
        isCustom: true,
        createdAt: Date.now(),
        updatedAt: Date.now(),
      })
    }

    const themeUtils = await import("@/lib/themeUtils")
    const themeData = themeUtils.createCustomThemeFromPreset(
      themeId as PresetThemeId,
      newName,
      `Based on ${themeId}`
    )
    return createCustomTheme(themeData)
  }

  const getTheme = useCallback(
    (id: string): Theme | undefined => {
      return PRESET_THEMES_DATA.find((t) => t.id === id) || customThemes.find((t) => t.id === id)
    },
    [customThemes]
  )

  const availableThemes = useMemo<Theme[]>(() => {
    return [...PRESET_THEMES_DATA, ...customThemes]
  }, [customThemes])

  const currentTheme = useMemo(() => {
    return resolveThemeCache(config.theme || "default", customThemes).id
  }, [config.theme, customThemes])

  return (
    <ThemeContext.Provider
      value={{
        currentTheme,
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

export type { Theme, CustomTheme, PresetTheme, PresetThemeId } from "@/types/theme"
export const PRESET_THEMES = PRESET_THEMES_DATA
