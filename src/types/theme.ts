// Theme types for custom theme management

export interface ThemeColors {
  background: string
  foreground: string
  card: string
  cardForeground: string
  popover: string
  popoverForeground: string
  primary: string
  primaryForeground: string
  secondary: string
  secondaryForeground: string
  muted: string
  mutedForeground: string
  accent: string
  accentForeground: string
  destructive: string
  destructiveForeground: string
  success: string
  successForeground: string
  warning: string
  warningForeground: string
  border: string
  input: string
  ring: string
  tabBackground: string
  tabActive: string
  tabActiveBorder: string
  tabHover: string
  titlebar: string
}

export interface TerminalPalette {
  background: string
  foreground: string
  cursor: string
  /** Text under a block cursor; xterm picks one when unset. */
  cursorAccent?: string
  selectionBackground: string
  /** Selected text; unset keeps each cell's own color. */
  selectionForeground?: string
  black: string
  red: string
  green: string
  yellow: string
  blue: string
  magenta: string
  cyan: string
  white: string
  brightBlack: string
  brightRed: string
  brightGreen: string
  brightYellow: string
  brightBlue: string
  brightMagenta: string
  brightCyan: string
  brightWhite: string
}

export const THEME_COLOR_KEYS = [
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
  "tabActiveBorder",
  "tabHover",
  "titlebar",
] as const satisfies ReadonlyArray<keyof ThemeColors>

interface ThemeDefinition {
  id: string
  name: string
  description?: string
  colors: ThemeColors
  terminal: TerminalPalette
}

export type ColorsAndPalette = Pick<ThemeDefinition, "colors" | "terminal">

export interface CustomTheme extends ThemeDefinition {
  baseTheme?: string // Which preset theme this is based on
  isCustom: true
  createdAt: number
  updatedAt: number
}

export interface PresetTheme extends ThemeDefinition {
  isCustom: false
}

/** A theme from the bundled theme library (see `@/lib/themeCatalog`). */
export interface CatalogTheme extends ThemeDefinition {
  isCustom: false
  isCatalog: true
  isDark: boolean
}

export type Theme = PresetTheme | CustomTheme | CatalogTheme

/** Which system appearance a theme is shown in, when themes follow the system. */
export type ThemeSlot = "light" | "dark"

export const PRESET_THEME_IDS = ["default", "light", "ocean", "forest", "sunset", "ubuntu"] as const

export type PresetThemeId = (typeof PRESET_THEME_IDS)[number]
