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
  tabHover: string
  titlebar: string
}

export interface CustomTheme {
  id: string
  name: string
  description?: string
  colors: ThemeColors
  baseTheme?: string // 基于哪个预设主题修改的
  isCustom: true
  createdAt: number
  updatedAt: number
}

export interface PresetTheme {
  id: string
  name: string
  description?: string
  isCustom: false
}

export type Theme = PresetTheme | CustomTheme

export const PRESET_THEME_IDS = ["default", "light", "ocean", "forest", "sunset", "ubuntu"] as const

export type PresetThemeId = (typeof PRESET_THEME_IDS)[number]
