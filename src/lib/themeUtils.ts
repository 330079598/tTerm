import { resolveThemeDefinition } from "@/lib/themeDefinitions"
import { applyThemeToDom, resolveThemeCache } from "@/lib/themePreloader"
import type { CustomTheme, PresetThemeId, TerminalPalette, ThemeColors } from "@/types/theme"

function cloneColors(colors: ThemeColors): ThemeColors {
  return { ...colors }
}

function cloneTerminalPalette(terminal: TerminalPalette): TerminalPalette {
  return { ...terminal }
}

export function createCustomThemeFromPreset(
  presetId: PresetThemeId,
  name: string,
  description?: string
): Omit<CustomTheme, "id"> {
  const baseTheme = resolveThemeDefinition(presetId, [])

  return {
    name,
    description,
    colors: cloneColors(baseTheme.colors),
    terminal: cloneTerminalPalette(baseTheme.terminal),
    baseTheme: presetId,
    isCustom: true,
    createdAt: Date.now(),
    updatedAt: Date.now(),
  }
}

export function previewCustomTheme(colors: ThemeColors) {
  applyThemeToDom(
    resolveThemeCache("temporary-preview", [
      {
        id: "temporary-preview",
        name: "Temporary Preview",
        colors,
        terminal: resolveThemeDefinition("default", []).terminal,
        isCustom: true,
        createdAt: 0,
        updatedAt: 0,
      },
    ])
  )
}

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

export function hslToCssColor(hsl: string): string {
  return `hsl(${hsl})`
}
