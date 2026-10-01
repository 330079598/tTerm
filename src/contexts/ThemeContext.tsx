import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
} from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { useConfig } from "@/contexts/ConfigContext"
import { announceThemeReady } from "@/lib/startup"
import { onSyncApplied } from "@/lib/sync"
import { getPresetTheme, PRESET_THEMES, resolveThemeDefinition } from "@/lib/themeDefinitions"
import { applyThemeToDom, cacheTheme, resolveThemeCache } from "@/lib/themePreloader"
import type { CustomTheme, PresetTheme, PresetThemeId, Theme, TerminalPalette } from "@/types/theme"
import { PRESET_THEME_IDS } from "@/types/theme"

/** Where themes were kept before the database; read once to hand them over. */
const LEGACY_STORAGE_KEY = "custom-themes"

function isPresetThemeId(themeId: string): themeId is PresetThemeId {
  return PRESET_THEME_IDS.includes(themeId as PresetThemeId)
}

function mergePresetThemeWithOverride(
  presetTheme: PresetTheme,
  overrideTheme: CustomTheme | undefined
): PresetTheme {
  if (!overrideTheme) {
    return presetTheme
  }

  return {
    ...presetTheme,
    name: overrideTheme.name,
    description: overrideTheme.description,
    colors: { ...overrideTheme.colors },
    terminal: { ...overrideTheme.terminal },
  }
}

interface ThemeContextType {
  currentTheme: string
  availableThemes: Theme[]
  presetThemes: PresetTheme[]
  customThemes: CustomTheme[]
  presetThemeOverrides: CustomTheme[]
  setTheme: (themeId: string) => Promise<void>
  createCustomTheme: (theme: Omit<CustomTheme, "id">) => Promise<CustomTheme>
  updateCustomTheme: (id: string, updates: Partial<CustomTheme>) => Promise<void>
  deleteCustomTheme: (id: string) => Promise<void>
  resetPresetTheme: (id: PresetThemeId) => Promise<void>
  duplicateTheme: (themeId: string, newName: string) => Promise<CustomTheme>
  getTheme: (id: string) => Theme | undefined
  /** Re-reads the saved themes, e.g. after a backup was imported. */
  reloadCustomThemes: () => Promise<void>
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined)

function fallbackTerminalPalette(baseTheme?: string): TerminalPalette {
  return { ...resolveThemeDefinition(baseTheme || "default", []).terminal }
}

function fallbackThemeColors(baseTheme?: string): PresetTheme["colors"] {
  return { ...resolveThemeDefinition(baseTheme || "default", []).colors }
}

function normalizeCustomTheme(rawTheme: unknown): CustomTheme | null {
  if (!rawTheme || typeof rawTheme !== "object") {
    return null
  }

  const theme = rawTheme as Partial<CustomTheme>
  if (!theme.id || !theme.name || !theme.colors) {
    return null
  }

  const fallbackColors = fallbackThemeColors(theme.baseTheme)

  return {
    id: theme.id,
    name: theme.name,
    description: theme.description,
    colors: { ...fallbackColors, ...theme.colors },
    terminal: theme.terminal ? { ...theme.terminal } : fallbackTerminalPalette(theme.baseTheme),
    baseTheme: theme.baseTheme,
    isCustom: true,
    createdAt: theme.createdAt ?? Date.now(),
    updatedAt: theme.updatedAt ?? Date.now(),
  }
}

function normalizeCustomThemes(themes: unknown[]): CustomTheme[] {
  return themes.map(normalizeCustomTheme).filter((theme): theme is CustomTheme => theme !== null)
}

/** The themes older versions kept in localStorage; `null` when there are none. */
function readLegacyThemes(): CustomTheme[] | null {
  try {
    const stored = localStorage.getItem(LEGACY_STORAGE_KEY)
    if (!stored) return null
    const parsed = JSON.parse(stored) as unknown
    return Array.isArray(parsed) ? normalizeCustomThemes(parsed) : null
  } catch (error) {
    console.error("Failed to read the themes kept in localStorage:", error)
    return null
  }
}

function removeLegacyThemes() {
  try {
    localStorage.removeItem(LEGACY_STORAGE_KEY)
  } catch {
    // Left behind, it is only read again if the database lost its themes.
  }
}

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation()
  const { config, updateTheme, isLoaded } = useConfig()
  const [customThemes, setCustomThemes] = useState<CustomTheme[]>([])
  const [themesLoaded, setThemesLoaded] = useState(false)

  const presetThemeOverrides = useMemo(
    () => customThemes.filter((theme) => isPresetThemeId(theme.id)),
    [customThemes]
  )

  const standaloneCustomThemes = useMemo(
    () => customThemes.filter((theme) => !isPresetThemeId(theme.id)),
    [customThemes]
  )

  const presetThemes = useMemo(
    () =>
      PRESET_THEMES.map((theme) =>
        mergePresetThemeWithOverride(
          theme,
          presetThemeOverrides.find((overrideTheme) => overrideTheme.id === theme.id)
        )
      ),
    [presetThemeOverrides]
  )

  const applyAndCacheTheme = useCallback((themeId: string, themes: CustomTheme[]) => {
    const themeCache = resolveThemeCache(themeId, themes)
    applyThemeToDom(themeCache)
    cacheTheme(themeCache)
    return themeCache.id
  }, [])

  const loadCustomThemes = useCallback(async () => {
    try {
      // The first load hands the themes kept in localStorage to the database.
      const themes = await invoke<unknown[]>("load_custom_themes", { legacy: readLegacyThemes() })
      setCustomThemes(normalizeCustomThemes(themes))
      removeLegacyThemes()
    } catch (error) {
      console.error("Failed to load custom themes:", error)
      setCustomThemes((current) => (current.length > 0 ? current : (readLegacyThemes() ?? [])))
    } finally {
      setThemesLoaded(true)
    }
  }, [])

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- state is only set after invoke() resolves or rejects
    void loadCustomThemes()
  }, [loadCustomThemes])

  useEffect(() => onSyncApplied(["themes"], () => void loadCustomThemes()), [loadCustomThemes])

  const saveCustomThemes = useCallback(async (themes: CustomTheme[]) => {
    try {
      await invoke("save_custom_themes", { themes })
      setCustomThemes(themes)
    } catch (error) {
      console.error("Failed to save custom themes:", error)
      throw error
    }
  }, [])

  useLayoutEffect(() => {
    // A custom theme is unknown until the themes load; applying it earlier
    // would show and cache the fallback.
    if (!isLoaded || !themesLoaded) return

    // An unknown theme shows the fallback without saving it: sync can deliver
    // the theme setting and the custom theme it names in separate steps, or
    // to a device that does not sync themes, and saving the fallback would
    // send it back to every other device.
    applyAndCacheTheme(config.theme || "default", customThemes)

    announceThemeReady()
  }, [applyAndCacheTheme, config.theme, customThemes, isLoaded, themesLoaded])

  const setTheme = useCallback(
    async (themeId: string): Promise<void> => {
      const resolvedThemeId = applyAndCacheTheme(themeId, customThemes)
      await updateTheme(resolvedThemeId)
    },
    [applyAndCacheTheme, customThemes, updateTheme]
  )

  const createCustomTheme = useCallback(
    async (theme: Omit<CustomTheme, "id">): Promise<CustomTheme> => {
      const newTheme: CustomTheme = {
        ...theme,
        id: `custom-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      }

      const updatedThemes = [...customThemes, newTheme]
      await saveCustomThemes(updatedThemes)

      return newTheme
    },
    [customThemes, saveCustomThemes]
  )

  const updateCustomTheme = useCallback(
    async (id: string, updates: Partial<CustomTheme>) => {
      const presetTheme = getPresetTheme(id)
      const existingTheme = customThemes.find((theme) => theme.id === id)

      let updatedThemes: CustomTheme[]

      if (existingTheme) {
        updatedThemes = customThemes.map((theme) =>
          theme.id === id ? { ...theme, ...updates, updatedAt: Date.now() } : theme
        )
      } else if (presetTheme) {
        const createdTheme: CustomTheme = {
          id,
          name: updates.name ?? presetTheme.name,
          description: updates.description ?? presetTheme.description,
          colors: updates.colors ? { ...updates.colors } : { ...presetTheme.colors },
          terminal: updates.terminal ? { ...updates.terminal } : { ...presetTheme.terminal },
          baseTheme: presetTheme.id,
          isCustom: true,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        }

        updatedThemes = [...customThemes, createdTheme]
      } else {
        return
      }

      await saveCustomThemes(updatedThemes)

      if (config.theme === id) {
        applyAndCacheTheme(id, updatedThemes)
      }
    },
    [applyAndCacheTheme, config.theme, customThemes, saveCustomThemes]
  )

  const deleteCustomTheme = useCallback(
    async (id: string) => {
      const updatedThemes = customThemes.filter((theme) => theme.id !== id)
      await saveCustomThemes(updatedThemes)

      if (config.theme === id) {
        await setTheme("default")
      }
    },
    [config.theme, customThemes, saveCustomThemes, setTheme]
  )

  const resetPresetTheme = useCallback(
    async (id: PresetThemeId) => {
      const updatedThemes = customThemes.filter((theme) => theme.id !== id)
      await saveCustomThemes(updatedThemes)

      if (config.theme === id) {
        applyAndCacheTheme(id, updatedThemes)
      }
    },
    [applyAndCacheTheme, config.theme, customThemes, saveCustomThemes]
  )

  const duplicateTheme = useCallback(
    async (themeId: string, newName: string): Promise<CustomTheme> => {
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
        t("theme.basedOn", { themeId, defaultValue: "Based on {{themeId}}" })
      )
      return createCustomTheme(themeData)
    },
    [createCustomTheme, customThemes, t]
  )

  const getTheme = useCallback(
    (id: string): Theme | undefined => {
      return (
        presetThemes.find((theme) => theme.id === id) ||
        standaloneCustomThemes.find((theme) => theme.id === id)
      )
    },
    [presetThemes, standaloneCustomThemes]
  )

  const availableThemes = useMemo<Theme[]>(() => {
    return [...presetThemes, ...standaloneCustomThemes]
  }, [presetThemes, standaloneCustomThemes])

  const currentTheme = useMemo(() => {
    return resolveThemeCache(config.theme || "default", customThemes).id
  }, [config.theme, customThemes])

  const contextValue = useMemo<ThemeContextType>(
    () => ({
      currentTheme,
      availableThemes,
      presetThemes,
      customThemes: standaloneCustomThemes,
      presetThemeOverrides,
      setTheme,
      createCustomTheme,
      updateCustomTheme,
      deleteCustomTheme,
      resetPresetTheme,
      duplicateTheme,
      getTheme,
      reloadCustomThemes: loadCustomThemes,
    }),
    [
      availableThemes,
      createCustomTheme,
      currentTheme,
      deleteCustomTheme,
      duplicateTheme,
      getTheme,
      loadCustomThemes,
      presetThemeOverrides,
      presetThemes,
      resetPresetTheme,
      setTheme,
      standaloneCustomThemes,
      updateCustomTheme,
    ]
  )

  return <ThemeContext.Provider value={contextValue}>{children}</ThemeContext.Provider>
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
