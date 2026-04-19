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
import { PRESET_THEMES, resolveThemeDefinition } from "@/lib/themeDefinitions"
import { applyThemeToDom, cacheTheme, resolveThemeCache } from "@/lib/themePreloader"
import type { CustomTheme, PresetTheme, PresetThemeId, Theme, TerminalPalette } from "@/types/theme"

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

function fallbackTerminalPalette(baseTheme?: string): TerminalPalette {
  return { ...resolveThemeDefinition(baseTheme || "default", []).terminal }
}

function normalizeCustomTheme(rawTheme: unknown): CustomTheme | null {
  if (!rawTheme || typeof rawTheme !== "object") {
    return null
  }

  const theme = rawTheme as Partial<CustomTheme>
  if (!theme.id || !theme.name || !theme.colors) {
    return null
  }

  return {
    id: theme.id,
    name: theme.name,
    description: theme.description,
    colors: theme.colors,
    terminal: theme.terminal ? { ...theme.terminal } : fallbackTerminalPalette(theme.baseTheme),
    baseTheme: theme.baseTheme,
    isCustom: true,
    createdAt: theme.createdAt ?? Date.now(),
    updatedAt: theme.updatedAt ?? Date.now(),
  }
}

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

  useEffect(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY)
      if (stored) {
        const themes = (JSON.parse(stored) as unknown[])
          .map(normalizeCustomTheme)
          .filter((theme): theme is CustomTheme => theme !== null)
        setCustomThemes(themes)
      }
    } catch (error) {
      console.error("Failed to load custom themes:", error)
    } finally {
      setThemesLoaded(true)
    }
  }, [])

  const saveCustomThemes = useCallback((themes: CustomTheme[]) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(themes))
      setCustomThemes(themes)
    } catch (error) {
      console.error("Failed to save custom themes:", error)
      throw error
    }
  }, [])

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
    const sourceTheme = customThemes.find((theme) => theme.id === themeId)

    if (sourceTheme) {
      return createCustomTheme({
        name: newName,
        description: sourceTheme.description,
        colors: { ...sourceTheme.colors },
        terminal: { ...sourceTheme.terminal },
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
      return (
        PRESET_THEMES.find((theme) => theme.id === id) ||
        customThemes.find((theme) => theme.id === id)
      )
    },
    [customThemes]
  )

  const availableThemes = useMemo<Theme[]>(() => {
    return [...PRESET_THEMES, ...customThemes]
  }, [customThemes])

  const currentTheme = useMemo(() => {
    return resolveThemeCache(config.theme || "default", customThemes).id
  }, [config.theme, customThemes])

  return (
    <ThemeContext.Provider
      value={{
        currentTheme,
        availableThemes,
        presetThemes: PRESET_THEMES,
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
export { PRESET_THEMES }
