import type { CustomTheme, TerminalPalette, ThemeColors } from "@/types/theme"

export interface ThemeEditorProps {
  themeId?: string
  baseThemeId?: string
  onClose: () => void
  onSave?: (theme: CustomTheme) => void
}

export type ThemeColorItem = {
  key: keyof ThemeColors
  labelKey: string
  token: string
  descriptionKey: string
}

export type TerminalColorItem = {
  /** The editor covers the colors every palette has. */
  key: {
    [Key in keyof TerminalPalette]-?: undefined extends TerminalPalette[Key] ? never : Key
  }[keyof TerminalPalette]
  labelKey: string
}

export type HslParts = { h: number; s: number; l: number }
