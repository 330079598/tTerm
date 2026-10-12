import { resolveThemeDefinition } from "@/lib/themeDefinitions"
import type { CustomTheme, PresetTheme, PresetThemeId, TerminalPalette } from "@/types/theme"
import { PRESET_THEME_IDS } from "@/types/theme"

/** Where themes were kept before the database; read once to hand them over. */
const LEGACY_STORAGE_KEY = "custom-themes"

export function isPresetThemeId(themeId: string): themeId is PresetThemeId {
  return PRESET_THEME_IDS.includes(themeId as PresetThemeId)
}

export function mergePresetThemeWithOverride(
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

export function normalizeCustomThemes(themes: unknown[]): CustomTheme[] {
  return themes.map(normalizeCustomTheme).filter((theme): theme is CustomTheme => theme !== null)
}

/** The themes older versions kept in localStorage; `null` when there are none. */
export function readLegacyThemes(): CustomTheme[] | null {
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

export function removeLegacyThemes() {
  try {
    localStorage.removeItem(LEGACY_STORAGE_KEY)
  } catch {
    // Left behind, it is only read again if the database lost its themes.
  }
}
