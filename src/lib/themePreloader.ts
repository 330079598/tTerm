import { getPresetTheme } from "@/lib/themeDefinitions"
import type { CustomTheme, ThemeColors, Theme as AppTheme } from "@/types/theme"
import { THEME_COLOR_KEYS } from "@/types/theme"

/**
 * Theme preloader utility
 *
 * Storage strategy:
 * 1. localStorage as fast cache (primary)
 * 2. Tauri config file as source of truth (backup)
 * 3. On startup: use localStorage first, then calibrate with Tauri config
 *
 * This dual-source approach ensures:
 * - Fast startup with cached theme (no flash)
 * - Reliability with Tauri config as fallback
 * - Auto-recovery if localStorage is cleared
 */

export interface ThemeCache {
  id: string
  isCustom: boolean
  colors?: ThemeColors
  timestamp?: number
}

const THEME_CACHE_KEY = "tterm-theme-cache"
const CACHE_VERSION_KEY = "tterm-cache-version"
const CURRENT_CACHE_VERSION = "2.0.0"

function toCssVariableName(key: keyof ThemeColors): string {
  return key.replace(/([A-Z])/g, "-$1").toLowerCase()
}

function toCssColor(value: string): string {
  const normalized = value.trim()

  if (
    normalized.startsWith("hsl(") ||
    normalized.startsWith("rgb(") ||
    normalized.startsWith("#") ||
    normalized.startsWith("var(")
  ) {
    return normalized
  }

  return `hsl(${normalized})`
}

function clearCustomThemeColors(): void {
  const root = document.documentElement

  THEME_COLOR_KEYS.forEach((key) => {
    root.style.removeProperty(`--${toCssVariableName(key)}`)
  })
}

function isLegacyTheme(themeId: string): themeId is AppTheme["id"] {
  return getPresetTheme(themeId) !== undefined
}

/**
 * Apply custom theme colors to DOM
 */
function applyCustomThemeColors(colors: ThemeColors): void {
  const root = document.documentElement

  Object.entries(colors).forEach(([key, value]) => {
    const cssVar = toCssVariableName(key as keyof ThemeColors)
    root.style.setProperty(`--${cssVar}`, value)
  })

  if (colors.background) {
    document.body.style.backgroundColor = toCssColor(colors.background)
  }
}

function applyPresetTheme(themeId: AppTheme["id"]): void {
  const preset = getPresetTheme(themeId)
  if (!preset) {
    return
  }

  document.documentElement.setAttribute("data-theme", themeId)

  Object.entries(preset.colors).forEach(([key, value]) => {
    const cssVar = toCssVariableName(key as keyof ThemeColors)
    document.documentElement.style.setProperty(`--${cssVar}`, value)
  })

  document.body.style.backgroundColor = toCssColor(preset.colors.background)
}

export function resolveThemeCache(themeId: string, customThemes: CustomTheme[]): ThemeCache {
  const customTheme = customThemes.find((theme) => theme.id === themeId)
  if (customTheme) {
    return {
      id: customTheme.id,
      isCustom: true,
      colors: customTheme.colors,
    }
  }

  if (isLegacyTheme(themeId)) {
    return {
      id: themeId,
      isCustom: false,
    }
  }

  return {
    id: "default",
    isCustom: false,
  }
}

export function applyThemeToDom(themeCache: ThemeCache): void {
  clearCustomThemeColors()

  if (themeCache.isCustom && themeCache.colors) {
    document.documentElement.removeAttribute("data-theme")
    applyCustomThemeColors(themeCache.colors)
    return
  }

  const presetThemeId = isLegacyTheme(themeCache.id) ? themeCache.id : "default"
  applyPresetTheme(presetThemeId)
}

/**
 * Apply default theme to DOM
 */
function applyDefaultTheme(): void {
  applyThemeToDom({
    id: "default",
    isCustom: false,
  })
}

/**
 * Validate cache version
 */
function isCacheValid(): boolean {
  try {
    const version = localStorage.getItem(CACHE_VERSION_KEY)
    return version === CURRENT_CACHE_VERSION
  } catch {
    return false
  }
}

/**
 * Validate theme cache data integrity
 */
function isThemeCacheValid(cache: unknown): cache is ThemeCache {
  if (!cache || typeof cache !== "object") return false

  const themeCache = cache as Partial<ThemeCache>

  if (!themeCache.id || typeof themeCache.id !== "string") return false
  if (typeof themeCache.isCustom !== "boolean") return false

  if (themeCache.isCustom && !themeCache.colors) return false
  if (!themeCache.isCustom && !isLegacyTheme(themeCache.id)) return false

  return true
}

/**
 * Preload theme from localStorage and apply to DOM
 * This function runs synchronously before React mounts
 *
 * @returns ThemeCache if successfully loaded, null otherwise
 */
export function preloadTheme(): ThemeCache | null {
  try {
    if (!isCacheValid()) {
      console.warn("[ThemePreloader] Cache version mismatch, using default theme")
      applyDefaultTheme()
      return null
    }

    const cached = localStorage.getItem(THEME_CACHE_KEY)
    if (!cached) {
      applyDefaultTheme()
      return null
    }

    const themeCache = JSON.parse(cached) as unknown

    if (!isThemeCacheValid(themeCache)) {
      console.warn("[ThemePreloader] Invalid theme cache data")
      applyDefaultTheme()
      return null
    }

    applyThemeToDom(themeCache)

    return themeCache
  } catch (error) {
    console.error("[ThemePreloader] Failed to preload theme:", error)
    applyDefaultTheme()
    return null
  }
}

/**
 * Cache theme configuration to localStorage
 *
 * @param themeCache - Theme cache data to store
 */
export function cacheTheme(themeCache: ThemeCache): void {
  try {
    const cacheWithTimestamp: ThemeCache = {
      ...themeCache,
      timestamp: Date.now(),
    }

    localStorage.setItem(THEME_CACHE_KEY, JSON.stringify(cacheWithTimestamp))
    localStorage.setItem(CACHE_VERSION_KEY, CURRENT_CACHE_VERSION)
  } catch (error) {
    console.error("[ThemePreloader] Failed to cache theme:", error)
  }
}

/**
 * Clear theme cache (for debugging or reset)
 */
export function clearThemeCache(): void {
  try {
    localStorage.removeItem(THEME_CACHE_KEY)
    localStorage.removeItem(CACHE_VERSION_KEY)
  } catch (error) {
    console.error("[ThemePreloader] Failed to clear theme cache:", error)
  }
}
