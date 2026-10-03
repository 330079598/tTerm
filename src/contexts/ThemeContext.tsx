import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { useConfig, type AppConfig } from "@/contexts/ConfigContext"
import { useSystemPrefersDark } from "@/hooks/useSystemPrefersDark"
import { announceThemeReady } from "@/lib/startup"
import { onSyncApplied } from "@/lib/sync"
import { createCatalogTheme, isCatalogThemeId, loadThemeCatalog } from "@/lib/themeCatalog"
import { getPresetTheme, PRESET_THEMES, resolveThemeDefinition } from "@/lib/themeDefinitions"
import { isDarkPalette } from "@/lib/themeDerivation"
import {
  applyThemeToDom,
  cacheTheme,
  readCachedTheme,
  resolveThemeCache,
} from "@/lib/themePreloader"
import type {
  CatalogTheme,
  CustomTheme,
  PresetTheme,
  PresetThemeId,
  Theme,
  ThemeSlot,
  TerminalPalette,
} from "@/types/theme"
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
  /** The theme on screen: the one being previewed, else the current one. */
  displayedTheme: string
  availableThemes: Theme[]
  presetThemes: PresetTheme[]
  customThemes: CustomTheme[]
  presetThemeOverrides: CustomTheme[]
  /**
   * Chooses a theme. While following the system it becomes the theme for its
   * own appearance, which is returned; it shows once the system is in that one.
   */
  setTheme: (themeId: string) => Promise<ThemeSlot | null>
  followSystem: boolean
  /** The system appearance, `null` until known. */
  systemPrefersDark: boolean | null
  lightTheme: string
  darkTheme: string
  setFollowSystem: (enabled: boolean) => Promise<void>
  setSystemTheme: (slot: ThemeSlot, themeId: string) => Promise<void>
  isDarkTheme: (themeId: string) => boolean
  favoriteThemes: string[]
  toggleFavoriteTheme: (themeId: string) => Promise<void>
  createCustomTheme: (theme: Omit<CustomTheme, "id">) => Promise<CustomTheme>
  /** Adds several themes in one save, e.g. the schemes of an imported file. */
  createCustomThemes: (themes: Array<Omit<CustomTheme, "id">>) => Promise<CustomTheme[]>
  updateCustomTheme: (id: string, updates: Partial<CustomTheme>) => Promise<void>
  deleteCustomTheme: (id: string) => Promise<void>
  resetPresetTheme: (id: PresetThemeId) => Promise<void>
  duplicateTheme: (themeId: string, newName: string) => Promise<CustomTheme>
  getTheme: (id: string) => Theme | undefined
  /** The theme library, once `loadCatalogThemes` has loaded it. */
  catalogThemes: CatalogTheme[] | null
  loadCatalogThemes: () => Promise<CatalogTheme[]>
  /** Shows a theme without choosing it; `null` goes back to the current one. */
  previewTheme: (themeId: string | null) => void
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
  const { config, saveConfig, isLoaded } = useConfig()
  const [customThemes, setCustomThemes] = useState<CustomTheme[]>([])
  // The latest themes for calls made right after a save, from callbacks that
  // still hold the themes before it.
  const customThemesRef = useRef<CustomTheme[]>([])
  const [themesLoaded, setThemesLoaded] = useState(false)
  const [catalogThemes, setCatalogThemes] = useState<CatalogTheme[] | null>(null)
  const [catalogFailed, setCatalogFailed] = useState(false)
  // The library theme the last session showed, rebuilt from its cached
  // palette so a start in it never loads the library.
  const [cachedCatalogTheme] = useState<CatalogTheme | null>(() => {
    const cache = readCachedTheme()
    return cache && isCatalogThemeId(cache.id) && cache.terminal
      ? createCatalogTheme(cache.id, cache.terminal)
      : null
  })
  const [previewThemeId, setPreviewThemeId] = useState<string | null>(null)
  const followSystem = config.theme_follow_system
  const systemPrefersDark = useSystemPrefersDark(followSystem)
  const configuredThemeId =
    (followSystem ? (systemPrefersDark ? config.theme_dark : config.theme_light) : config.theme) ||
    "default"
  // Which of the two themes to show is unknown until the system tells.
  const appearanceSettled = !followSystem || systemPrefersDark !== null

  const knownCatalogThemes = useMemo(
    () => catalogThemes ?? (cachedCatalogTheme ? [cachedCatalogTheme] : []),
    [cachedCatalogTheme, catalogThemes]
  )

  const loadCatalogThemes = useCallback(async () => {
    const themes = await loadThemeCatalog()
    setCatalogThemes(themes)
    setCatalogFailed(false)
    return themes
  }, [])

  // Only a library theme the cache cannot stand in for needs the library;
  // one the library lacks too shows the fallback once it has loaded.
  const needsCatalog =
    isCatalogThemeId(configuredThemeId) &&
    catalogThemes === null &&
    !knownCatalogThemes.some((theme) => theme.id === configuredThemeId)
  useEffect(() => {
    if (!needsCatalog) return
    // eslint-disable-next-line react-hooks/set-state-in-effect -- state is only set once the library loads or fails
    loadCatalogThemes().catch((error: unknown) => {
      console.error("Failed to load the theme library:", error)
      setCatalogFailed(true)
    })
  }, [loadCatalogThemes, needsCatalog])

  // Until then that theme is unknown.
  const catalogSettled = !needsCatalog || catalogFailed

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

  const applyAndCacheTheme = useCallback(
    (themeId: string, themes: CustomTheme[]) => {
      const themeCache = resolveThemeCache(themeId, themes, knownCatalogThemes)
      applyThemeToDom(themeCache)
      cacheTheme(themeCache)
      return themeCache.id
    },
    [knownCatalogThemes]
  )

  const loadCustomThemes = useCallback(async () => {
    try {
      // The first load hands the themes kept in localStorage to the database.
      const themes = await invoke<unknown[]>("load_custom_themes", { legacy: readLegacyThemes() })
      customThemesRef.current = normalizeCustomThemes(themes)
      setCustomThemes(customThemesRef.current)
      removeLegacyThemes()
    } catch (error) {
      console.error("Failed to load custom themes:", error)
      if (customThemesRef.current.length === 0) {
        customThemesRef.current = readLegacyThemes() ?? []
      }
      setCustomThemes(customThemesRef.current)
    } finally {
      setThemesLoaded(true)
    }
  }, [])

  useEffect(() => {
    void loadCustomThemes()
  }, [loadCustomThemes])

  useEffect(() => onSyncApplied(["themes"], () => void loadCustomThemes()), [loadCustomThemes])

  const saveCustomThemes = useCallback(async (themes: CustomTheme[]) => {
    try {
      await invoke("save_custom_themes", { themes })
      customThemesRef.current = themes
      setCustomThemes(themes)
    } catch (error) {
      console.error("Failed to save custom themes:", error)
      throw error
    }
  }, [])

  useLayoutEffect(() => {
    // A custom theme is unknown until the themes load; applying it earlier
    // would show and cache the fallback.
    if (!isLoaded || !themesLoaded || !catalogSettled || !appearanceSettled) return

    // A preview is shown but never cached: closing the app mid-preview
    // starts the next session in the chosen theme.
    if (previewThemeId) {
      applyThemeToDom(resolveThemeCache(previewThemeId, customThemes, knownCatalogThemes))
      return
    }

    // An unknown theme shows the fallback without saving it: sync can deliver
    // the theme setting and the custom theme it names in separate steps, or
    // to a device that does not sync themes, and saving the fallback would
    // send it back to every other device.
    applyAndCacheTheme(configuredThemeId, customThemes)

    announceThemeReady()
  }, [
    appearanceSettled,
    applyAndCacheTheme,
    catalogSettled,
    configuredThemeId,
    customThemes,
    isLoaded,
    knownCatalogThemes,
    previewThemeId,
    themesLoaded,
  ])

  const isDarkTheme = useCallback(
    (themeId: string) => {
      const theme =
        customThemesRef.current.find((candidate) => candidate.id === themeId) ??
        knownCatalogThemes.find((candidate) => candidate.id === themeId) ??
        getPresetTheme(themeId)
      return theme ? isDarkPalette(theme.terminal) : false
    },
    [knownCatalogThemes]
  )

  const setTheme = useCallback(
    async (themeId: string): Promise<ThemeSlot | null> => {
      const resolvedThemeId = resolveThemeCache(
        themeId,
        customThemesRef.current,
        knownCatalogThemes
      ).id
      const slot: ThemeSlot | null = followSystem
        ? isDarkTheme(resolvedThemeId)
          ? "dark"
          : "light"
        : null
      if (!slot || (slot === "dark") === systemPrefersDark) {
        applyAndCacheTheme(resolvedThemeId, customThemesRef.current)
      }
      const update: Partial<AppConfig> = { theme: resolvedThemeId }
      if (slot === "dark") update.theme_dark = resolvedThemeId
      if (slot === "light") update.theme_light = resolvedThemeId
      try {
        await saveConfig(update)
      } finally {
        // Only once the setting holds the new theme: ending a preview
        // earlier would show the old one in between.
        setPreviewThemeId(null)
      }
      return slot
    },
    [
      applyAndCacheTheme,
      followSystem,
      isDarkTheme,
      knownCatalogThemes,
      saveConfig,
      systemPrefersDark,
    ]
  )

  const setSystemTheme = useCallback(
    async (slot: ThemeSlot, themeId: string) => {
      await saveConfig(slot === "dark" ? { theme_dark: themeId } : { theme_light: themeId })
    },
    [saveConfig]
  )

  const favoriteThemes = config.favorite_themes
  // Starring twice before the first save lands must not lose a star.
  const favoritesRef = useRef(favoriteThemes)
  useEffect(() => {
    favoritesRef.current = favoriteThemes
  }, [favoriteThemes])

  const toggleFavoriteTheme = useCallback(
    async (themeId: string) => {
      const previous = favoritesRef.current
      favoritesRef.current = previous.includes(themeId)
        ? previous.filter((id) => id !== themeId)
        : [...previous, themeId]
      try {
        await saveConfig({ favorite_themes: favoritesRef.current })
      } catch (error) {
        favoritesRef.current = previous
        throw error
      }
    },
    [saveConfig]
  )

  const createCustomThemes = useCallback(
    async (themes: Array<Omit<CustomTheme, "id">>): Promise<CustomTheme[]> => {
      const newThemes: CustomTheme[] = themes.map((theme) => ({
        ...theme,
        id: `custom-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      }))

      await saveCustomThemes([...customThemesRef.current, ...newThemes])

      return newThemes
    },
    [saveCustomThemes]
  )

  const createCustomTheme = useCallback(
    async (theme: Omit<CustomTheme, "id">): Promise<CustomTheme> => {
      const [newTheme] = await createCustomThemes([theme])
      return newTheme
    },
    [createCustomThemes]
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

      // Every setting naming the theme falls back to a built-in one.
      const update: Partial<AppConfig> = {}
      if (config.theme === id) update.theme = "default"
      if (config.theme_light === id) update.theme_light = "light"
      if (config.theme_dark === id) update.theme_dark = "default"
      if (config.favorite_themes.includes(id)) {
        update.favorite_themes = config.favorite_themes.filter((themeId) => themeId !== id)
      }
      if (Object.keys(update).length > 0) await saveConfig(update)
    },
    [config, customThemes, saveConfig, saveCustomThemes]
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
      const catalogTheme = knownCatalogThemes.find((theme) => theme.id === themeId)

      if (catalogTheme) {
        return createCustomTheme({
          name: newName,
          description: t("theme.basedOn", { themeId: catalogTheme.name }),
          colors: { ...catalogTheme.colors },
          terminal: { ...catalogTheme.terminal },
          baseTheme: catalogTheme.id,
          isCustom: true,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        })
      }

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
    [createCustomTheme, customThemes, knownCatalogThemes, t]
  )

  const getTheme = useCallback(
    (id: string): Theme | undefined => {
      return (
        presetThemes.find((theme) => theme.id === id) ||
        standaloneCustomThemes.find((theme) => theme.id === id) ||
        knownCatalogThemes.find((theme) => theme.id === id)
      )
    },
    [knownCatalogThemes, presetThemes, standaloneCustomThemes]
  )

  const availableThemes = useMemo<Theme[]>(() => {
    return [...presetThemes, ...standaloneCustomThemes]
  }, [presetThemes, standaloneCustomThemes])

  const currentTheme = useMemo(() => {
    return resolveThemeCache(configuredThemeId, customThemes, knownCatalogThemes).id
  }, [configuredThemeId, customThemes, knownCatalogThemes])

  const setFollowSystem = useCallback(
    async (enabled: boolean) => {
      // Turned on, the current theme becomes the one for its appearance;
      // turned off, the theme on screen is the one shown from now on.
      await saveConfig(
        enabled
          ? {
              theme_follow_system: true,
              ...(isDarkTheme(currentTheme)
                ? { theme_dark: currentTheme }
                : { theme_light: currentTheme }),
            }
          : { theme_follow_system: false, theme: currentTheme }
      )
    },
    [currentTheme, isDarkTheme, saveConfig]
  )

  const displayedTheme = useMemo(() => {
    return previewThemeId
      ? resolveThemeCache(previewThemeId, customThemes, knownCatalogThemes).id
      : currentTheme
  }, [currentTheme, customThemes, knownCatalogThemes, previewThemeId])

  const contextValue = useMemo<ThemeContextType>(
    () => ({
      currentTheme,
      displayedTheme,
      followSystem,
      systemPrefersDark,
      lightTheme: config.theme_light,
      darkTheme: config.theme_dark,
      setFollowSystem,
      setSystemTheme,
      isDarkTheme,
      favoriteThemes,
      toggleFavoriteTheme,
      availableThemes,
      presetThemes,
      customThemes: standaloneCustomThemes,
      presetThemeOverrides,
      setTheme,
      createCustomTheme,
      createCustomThemes,
      updateCustomTheme,
      deleteCustomTheme,
      resetPresetTheme,
      duplicateTheme,
      getTheme,
      catalogThemes,
      loadCatalogThemes,
      previewTheme: setPreviewThemeId,
      reloadCustomThemes: loadCustomThemes,
    }),
    [
      availableThemes,
      catalogThemes,
      config.theme_dark,
      config.theme_light,
      favoriteThemes,
      followSystem,
      isDarkTheme,
      setFollowSystem,
      setSystemTheme,
      systemPrefersDark,
      toggleFavoriteTheme,
      createCustomTheme,
      createCustomThemes,
      currentTheme,
      deleteCustomTheme,
      displayedTheme,
      duplicateTheme,
      getTheme,
      loadCatalogThemes,
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
