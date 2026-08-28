import { resolveThemeDefinition } from "@/lib/themeDefinitions"
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
