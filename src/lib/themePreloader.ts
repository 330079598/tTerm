import { invoke, isTauri } from "@tauri-apps/api/core"
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow"
import {
  getDetectedPlatform,
  isWindowBlurSupported,
  type WindowBlurMaterial,
} from "@/contexts/ConfigContext"
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

function isLightBackground(background: string): boolean {
  const normalized = background.trim()
  const hslMatch = normalized.match(/(\d+\.?\d*)\s+(\d+\.?\d*)%\s+(\d+\.?\d*)%/)
  if (hslMatch) {
    return parseFloat(hslMatch[3]) > 50
  }

  if (normalized.startsWith("#")) {
    const hex = normalized.slice(1)
    const r = parseInt(hex.slice(0, 2), 16)
    const g = parseInt(hex.slice(2, 4), 16)
    const b = parseInt(hex.slice(4, 6), 16)
    const luminance = 0.299 * r + 0.587 * g + 0.114 * b
    return luminance > 128
  }

  return false
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

export interface WindowBlur {
  enabled: boolean
  /** Blur radius in points (macOS). */
  radius: number
  /** Windows backdrop. */
  material: WindowBlurMaterial
  /** Opacity of the theme tint over the blur, 0-1. */
  opacity: number
}

declare global {
  interface Window {
    /** Tint opacity, set by the backend when the window was created with the blur on. */
    __TTERM_WINDOW_BLUR__?: number
  }
}

function readLaunchBlur(): WindowBlur {
  const opacity = typeof window === "undefined" ? undefined : window.__TTERM_WINDOW_BLUR__
  return typeof opacity === "number"
    ? { enabled: true, radius: 0, material: "acrylic", opacity }
    : { enabled: false, radius: 0, material: "acrylic", opacity: 1 }
}

let windowBlur = readLaunchBlur()
// The backend applied the configured blur at launch; the page only learns the
// details once the config loads and leaves the native window alone until then.
let windowBlurKnown = !windowBlur.enabled
let nativeBackground = ""
let nativeBlur = windowBlur.enabled ? "" : "off"

function applyWindowBlurToDom(): void {
  const root = document.documentElement
  root.classList.toggle("window-blur", windowBlur.enabled)
  if (windowBlur.enabled) {
    root.style.setProperty("--window-opacity", String(windowBlur.opacity))
  } else {
    root.style.removeProperty("--window-opacity")
  }
}

/**
 * The theme background as RGB. The theme sets it inline on the body; the
 * computed color is only a fallback because the blur styles clear it.
 */
function readThemeBackground(): [number, number, number] | null {
  const color =
    document.body.style.backgroundColor || getComputedStyle(document.body).backgroundColor
  const match = color.match(/\d+(\.\d+)?/g)
  if (!match || match.length < 3) return null
  const [r, g, b] = match.slice(0, 3).map((channel) => Math.round(Number(channel)))
  return [r, g, b]
}

function isDarkBackground([r, g, b]: [number, number, number]): boolean {
  return 0.299 * r + 0.587 * g + 0.114 * b <= 128
}

/** Windows backdrops follow the window theme, so it tracks the page's. */
function syncNativeBlur(): void {
  const detected = getDetectedPlatform()
  if (!isTauri() || !windowBlurKnown || (detected !== "macos" && detected !== "windows")) return
  const background = readThemeBackground()
  const dark = background ? isDarkBackground(background) : true
  const { enabled, radius, material } = windowBlur
  const key = !enabled
    ? "off"
    : detected === "macos"
      ? `radius:${radius}`
      : `${material}:${dark ? "dark" : "light"}`
  if (key === nativeBlur) return
  nativeBlur = key
  invoke("set_window_blur", { enabled, radius, material, dark }).catch((error: unknown) => {
    nativeBlur = ""
    console.error("[ThemePreloader] Failed to set window blur:", error)
  })
}

/**
 * Switch the blur behind the window. The page tints it with the theme
 * background at `opacity`; on macOS the native window drops its own
 * background so the blur shows through (the Windows window is always clear).
 */
export function setWindowBlur(next: WindowBlur): void {
  const enabled = next.enabled && isWindowBlurSupported()
  const blur: WindowBlur = enabled
    ? { ...next, enabled }
    : { enabled: false, radius: 0, material: next.material, opacity: 1 }
  const unchanged =
    windowBlurKnown &&
    blur.enabled === windowBlur.enabled &&
    blur.radius === windowBlur.radius &&
    blur.material === windowBlur.material &&
    blur.opacity === windowBlur.opacity
  windowBlur = blur
  windowBlurKnown = true
  if (unchanged) return
  applyWindowBlurToDom()
  syncNativeBackground()
  syncNativeBlur()
}

/**
 * Paint the native window and webview in the theme background. WebKit drops a
 * hidden page's layers; until it repaints after the window comes back,
 * whatever is behind the page shows, which is white unless set here. Not on
 * Windows: that window is transparent and must stay that way. The color is
 * also saved so the next launch starts in it. With the blur on, the window
 * stays clear so the blur shows through.
 */
function syncNativeBackground(): void {
  if (!isTauri()) return
  const background = readThemeBackground()
  if (!background) return
  const [r, g, b] = background
  const blur = windowBlur.enabled
  const key = `${r},${g},${b},${blur}`
  if (key === nativeBackground) return
  nativeBackground = key
  const onError = (error: unknown) => {
    nativeBackground = ""
    console.error("[ThemePreloader] Failed to sync native background:", error)
  }
  const hex = `#${[r, g, b].map((channel) => channel.toString(16).padStart(2, "0")).join("")}`
  invoke("save_window_background", { color: hex }).catch(onError)
  if (getDetectedPlatform() !== "windows") {
    getCurrentWebviewWindow()
      .setBackgroundColor([r, g, b, blur ? 0 : 255])
      .catch(onError)
  }
}

export function applyThemeToDom(themeCache: ThemeCache): void {
  applyThemeColors(themeCache)
  syncNativeBackground()
  syncNativeBlur()
}

function applyThemeColors(themeCache: ThemeCache): void {
  clearCustomThemeColors()

  if (themeCache.isCustom && themeCache.colors) {
    if (themeCache.colors.background && isLightBackground(themeCache.colors.background)) {
      document.documentElement.setAttribute("data-theme", "light")
    } else {
      document.documentElement.removeAttribute("data-theme")
    }
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
  applyWindowBlurToDom()
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
