import type { CustomTheme, ThemeColors } from "@/types/theme"
import { PRESET_THEME_IDS } from "@/types/theme"

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

const THEME_COLOR_KEYS = [
  "background",
  "foreground",
  "card",
  "cardForeground",
  "popover",
  "popoverForeground",
  "primary",
  "primaryForeground",
  "secondary",
  "secondaryForeground",
  "muted",
  "mutedForeground",
  "accent",
  "accentForeground",
  "destructive",
  "destructiveForeground",
  "success",
  "successForeground",
  "warning",
  "warningForeground",
  "border",
  "input",
  "ring",
  "tabBackground",
  "tabActive",
  "tabHover",
  "titlebar",
] as const satisfies ReadonlyArray<keyof ThemeColors>

const THEME_CACHE_KEY = "tterm-theme-cache"
const CACHE_VERSION_KEY = "tterm-cache-version"
const CURRENT_CACHE_VERSION = "1.0.0"

// Preset theme background colors (from themes.css)
const PRESET_THEME_BACKGROUNDS: Record<(typeof PRESET_THEME_IDS)[number], string> = {
  default: "hsl(220 13% 12%)",
  light: "hsl(0 0% 100%)",
  ocean: "hsl(200 30% 10%)",
  forest: "hsl(140 25% 12%)",
  sunset: "hsl(20 30% 12%)",
  ubuntu: "hsl(302 58% 10%)",
}

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

export function isPresetThemeId(themeId: string): themeId is (typeof PRESET_THEME_IDS)[number] {
  return PRESET_THEME_IDS.includes(themeId as (typeof PRESET_THEME_IDS)[number])
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

/**
 * Apply preset theme background to body
 */
function applyPresetThemeBackground(themeId: (typeof PRESET_THEME_IDS)[number]): void {
  document.body.style.backgroundColor = PRESET_THEME_BACKGROUNDS[themeId]
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

  if (isPresetThemeId(themeId)) {
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

  const presetThemeId = isPresetThemeId(themeCache.id) ? themeCache.id : "default"
  document.documentElement.setAttribute("data-theme", presetThemeId)
  applyPresetThemeBackground(presetThemeId)
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

  // Required fields
  if (!themeCache.id || typeof themeCache.id !== "string") return false
  if (typeof themeCache.isCustom !== "boolean") return false

  // If custom theme, must have colors
  if (themeCache.isCustom && !themeCache.colors) return false
  if (!themeCache.isCustom && !isPresetThemeId(themeCache.id)) return false

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
    // Check cache version
    if (!isCacheValid()) {
      console.warn("[ThemePreloader] Cache version mismatch, using default theme")
      applyDefaultTheme()
      return null
    }

    // Load cached theme
    const cached = localStorage.getItem(THEME_CACHE_KEY)
    if (!cached) {
      applyDefaultTheme()
      return null
    }

    const themeCache = JSON.parse(cached) as unknown

    // Validate cache data
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
    // localStorage failure doesn't affect app functionality
    // Tauri config file remains the source of truth
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
