import type { CustomTheme, PresetThemeId, ThemeColors } from "@/types/theme"
import { applyThemeToDom, resolveThemeCache } from "@/lib/themePreloader"

// Extract current theme color values from CSS variables
export function extractThemeColors(): ThemeColors {
  const root = document.documentElement
  const style = getComputedStyle(root)

  const getVar = (name: string): string => {
    return style.getPropertyValue(`--${name}`).trim()
  }

  return {
    background: getVar("background"),
    foreground: getVar("foreground"),
    card: getVar("card"),
    cardForeground: getVar("card-foreground"),
    popover: getVar("popover"),
    popoverForeground: getVar("popover-foreground"),
    primary: getVar("primary"),
    primaryForeground: getVar("primary-foreground"),
    secondary: getVar("secondary"),
    secondaryForeground: getVar("secondary-foreground"),
    muted: getVar("muted"),
    mutedForeground: getVar("muted-foreground"),
    accent: getVar("accent"),
    accentForeground: getVar("accent-foreground"),
    destructive: getVar("destructive"),
    destructiveForeground: getVar("destructive-foreground"),
    success: getVar("success"),
    successForeground: getVar("success-foreground"),
    warning: getVar("warning"),
    warningForeground: getVar("warning-foreground"),
    border: getVar("border"),
    input: getVar("input"),
    ring: getVar("ring"),
    tabBackground: getVar("tab-background"),
    tabActive: getVar("tab-active"),
    tabHover: getVar("tab-hover"),
    titlebar: getVar("titlebar"),
  }
}

// Apply custom theme colors to DOM
export function applyCustomTheme(colors: ThemeColors) {
  const root = document.documentElement

  Object.entries(colors).forEach(([key, value]) => {
    // Convert camelCase to kebab-case
    const cssVar = key.replace(/([A-Z])/g, "-$1").toLowerCase()
    root.style.setProperty(`--${cssVar}`, value)
  })
}

// Generate unique theme ID
export function generateThemeId(): string {
  return `custom-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`
}

// Create custom theme from preset theme
export function createCustomThemeFromPreset(
  presetId: PresetThemeId,
  name: string,
  description?: string
): Omit<CustomTheme, "id"> {
  const currentThemeId = document.documentElement.getAttribute("data-theme")
  const currentColors = extractThemeColors()

  applyThemeToDom(resolveThemeCache(presetId, []))
  const colors = extractThemeColors()

  if (currentThemeId) {
    applyThemeToDom({
      id: currentThemeId,
      isCustom: false,
    })
  } else {
    applyThemeToDom({
      id: "temporary-custom-theme",
      isCustom: true,
      colors: currentColors,
    })
  }

  return {
    name,
    description,
    colors,
    baseTheme: presetId,
    isCustom: true,
    createdAt: Date.now(),
    updatedAt: Date.now(),
  }
}

// HSL color conversion helper function
export function hslToString(h: number, s: number, l: number): string {
  return `${h} ${s}% ${l}%`
}

export function parseHsl(hsl: string): { h: number; s: number; l: number } | null {
  const match = hsl.match(/(\d+\.?\d*)\s+(\d+\.?\d*)%?\s+(\d+\.?\d*)%?/)
  if (!match) return null

  return {
    h: parseFloat(match[1]),
    s: parseFloat(match[2]),
    l: parseFloat(match[3]),
  }
}

// Color preview helper function
export function hslToCssColor(hsl: string): string {
  return `hsl(${hsl})`
}
