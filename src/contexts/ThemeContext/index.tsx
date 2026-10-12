import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
} from "react"
import { useTranslation } from "react-i18next"

import { useConfig } from "@/contexts/ConfigContext"
import type { AppConfig } from "@/lib/appConfig"
import { useSystemPrefersDark } from "@/hooks/useSystemPrefersDark"
import { announceThemeReady } from "@/lib/startup"
import { isCatalogThemeId } from "@/lib/themeCatalog"
import { getPresetTheme } from "@/lib/themeDefinitions"
import { isDarkPalette } from "@/lib/themeDerivation"
import { applyThemeToDom, cacheTheme, resolveThemeCache } from "@/lib/themePreloader"
import type {
  CatalogTheme,
  CustomTheme,
  PresetTheme,
  PresetThemeId,
  Theme,
  ThemeSlot,
} from "@/types/theme"
import { useCustomThemeActions } from "@/contexts/ThemeContext/useCustomThemeActions"
import { useCustomThemeStore } from "@/contexts/ThemeContext/useCustomThemeStore"
import { useFavoriteThemes } from "@/contexts/ThemeContext/useFavoriteThemes"
import { useThemeCatalog } from "@/contexts/ThemeContext/useThemeCatalog"

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

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation()
  const { config, saveConfig, isLoaded } = useConfig()
  const [previewThemeId, setPreviewThemeId] = useState<string | null>(null)
  const followSystem = config.theme_follow_system
  const systemPrefersDark = useSystemPrefersDark(followSystem)
  const configuredThemeId =
    (followSystem ? (systemPrefersDark ? config.theme_dark : config.theme_light) : config.theme) ||
    "default"
  // Which of the two themes to show is unknown until the system tells.
  const appearanceSettled = !followSystem || systemPrefersDark !== null

  const { catalogThemes, knownCatalogThemes, loadCatalogThemes, catalogSettled } =
    useThemeCatalog(configuredThemeId)
  const {
    customThemes,
    customThemesRef,
    themesLoaded,
    presetThemeOverrides,
    standaloneCustomThemes,
    presetThemes,
    loadCustomThemes,
    saveCustomThemes,
  } = useCustomThemeStore()

  const applyAndCacheTheme = useCallback(
    (themeId: string, themes: CustomTheme[]) => {
      const themeCache = resolveThemeCache(themeId, themes, knownCatalogThemes)
      applyThemeToDom(themeCache)
      cacheTheme(themeCache)
      return themeCache.id
    },
    [knownCatalogThemes]
  )

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
    [customThemesRef, knownCatalogThemes]
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
      customThemesRef,
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
  const toggleFavoriteTheme = useFavoriteThemes(favoriteThemes, saveConfig)

  const {
    createCustomThemes,
    createCustomTheme,
    updateCustomTheme,
    deleteCustomTheme,
    resetPresetTheme,
    duplicateTheme,
  } = useCustomThemeActions({
    customThemes,
    customThemesRef,
    saveCustomThemes,
    applyAndCacheTheme,
    config,
    saveConfig,
    knownCatalogThemes,
    t,
  })

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

/** Loads the theme library when one of `themeIds` comes from it. */
export function useCatalogFor(themeIds: readonly (string | undefined)[]) {
  const { catalogThemes, loadCatalogThemes } = useTheme()
  const needed =
    catalogThemes === null && themeIds.some((themeId) => !!themeId && isCatalogThemeId(themeId))
  useEffect(() => {
    if (!needed) return
    loadCatalogThemes().catch((error: unknown) => {
      console.error("Failed to load the theme library:", error)
    })
  }, [loadCatalogThemes, needed])
}
