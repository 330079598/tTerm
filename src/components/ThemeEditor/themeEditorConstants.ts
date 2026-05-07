import type { TerminalPalette } from "@/types/theme"

import type { TerminalColorItem, ThemeColorItem } from "@/components/ThemeEditor/types"

export const COLOR_GROUPS = {
  basic: [
    {
      key: "background",
      labelKey: "themeEditor.colors.background.label",
      token: "background",
      descriptionKey: "themeEditor.colors.background.description",
    },
    {
      key: "foreground",
      labelKey: "themeEditor.colors.foreground.label",
      token: "foreground",
      descriptionKey: "themeEditor.colors.foreground.description",
    },
    {
      key: "border",
      labelKey: "themeEditor.colors.border.label",
      token: "border",
      descriptionKey: "themeEditor.colors.border.description",
    },
  ],
  card: [
    {
      key: "card",
      labelKey: "themeEditor.colors.card.label",
      token: "card",
      descriptionKey: "themeEditor.colors.card.description",
    },
    {
      key: "cardForeground",
      labelKey: "themeEditor.colors.cardForeground.label",
      token: "cardForeground",
      descriptionKey: "themeEditor.colors.cardForeground.description",
    },
    {
      key: "popover",
      labelKey: "themeEditor.colors.popover.label",
      token: "popover",
      descriptionKey: "themeEditor.colors.popover.description",
    },
    {
      key: "popoverForeground",
      labelKey: "themeEditor.colors.popoverForeground.label",
      token: "popoverForeground",
      descriptionKey: "themeEditor.colors.popoverForeground.description",
    },
  ],
  primary: [
    {
      key: "primary",
      labelKey: "themeEditor.colors.primary.label",
      token: "primary",
      descriptionKey: "themeEditor.colors.primary.description",
    },
    {
      key: "primaryForeground",
      labelKey: "themeEditor.colors.primaryForeground.label",
      token: "primaryForeground",
      descriptionKey: "themeEditor.colors.primaryForeground.description",
    },
  ],
  secondary: [
    {
      key: "secondary",
      labelKey: "themeEditor.colors.secondary.label",
      token: "secondary",
      descriptionKey: "themeEditor.colors.secondary.description",
    },
    {
      key: "secondaryForeground",
      labelKey: "themeEditor.colors.secondaryForeground.label",
      token: "secondaryForeground",
      descriptionKey: "themeEditor.colors.secondaryForeground.description",
    },
  ],
  accent: [
    {
      key: "accent",
      labelKey: "themeEditor.colors.accent.label",
      token: "accent",
      descriptionKey: "themeEditor.colors.accent.description",
    },
    {
      key: "accentForeground",
      labelKey: "themeEditor.colors.accentForeground.label",
      token: "accentForeground",
      descriptionKey: "themeEditor.colors.accentForeground.description",
    },
  ],
  muted: [
    {
      key: "muted",
      labelKey: "themeEditor.colors.muted.label",
      token: "muted",
      descriptionKey: "themeEditor.colors.muted.description",
    },
    {
      key: "mutedForeground",
      labelKey: "themeEditor.colors.mutedForeground.label",
      token: "mutedForeground",
      descriptionKey: "themeEditor.colors.mutedForeground.description",
    },
  ],
  status: [
    {
      key: "destructive",
      labelKey: "themeEditor.colors.destructive.label",
      token: "destructive",
      descriptionKey: "themeEditor.colors.destructive.description",
    },
    {
      key: "destructiveForeground",
      labelKey: "themeEditor.colors.destructiveForeground.label",
      token: "destructiveForeground",
      descriptionKey: "themeEditor.colors.destructiveForeground.description",
    },
    {
      key: "success",
      labelKey: "themeEditor.colors.success.label",
      token: "success",
      descriptionKey: "themeEditor.colors.success.description",
    },
    {
      key: "successForeground",
      labelKey: "themeEditor.colors.successForeground.label",
      token: "successForeground",
      descriptionKey: "themeEditor.colors.successForeground.description",
    },
    {
      key: "warning",
      labelKey: "themeEditor.colors.warning.label",
      token: "warning",
      descriptionKey: "themeEditor.colors.warning.description",
    },
    {
      key: "warningForeground",
      labelKey: "themeEditor.colors.warningForeground.label",
      token: "warningForeground",
      descriptionKey: "themeEditor.colors.warningForeground.description",
    },
  ],
  input: [
    {
      key: "input",
      labelKey: "themeEditor.colors.input.label",
      token: "input",
      descriptionKey: "themeEditor.colors.input.description",
    },
    {
      key: "ring",
      labelKey: "themeEditor.colors.ring.label",
      token: "ring",
      descriptionKey: "themeEditor.colors.ring.description",
    },
  ],
  tabs: [
    {
      key: "tabBackground",
      labelKey: "themeEditor.colors.tabBackground.label",
      token: "tabBackground",
      descriptionKey: "themeEditor.colors.tabBackground.description",
    },
    {
      key: "tabActive",
      labelKey: "themeEditor.colors.tabActive.label",
      token: "tabActive",
      descriptionKey: "themeEditor.colors.tabActive.description",
    },
    {
      key: "tabActiveBorder",
      labelKey: "themeEditor.colors.tabActiveBorder.label",
      token: "tabActiveBorder",
      descriptionKey: "themeEditor.colors.tabActiveBorder.description",
    },
    {
      key: "tabHover",
      labelKey: "themeEditor.colors.tabHover.label",
      token: "tabHover",
      descriptionKey: "themeEditor.colors.tabHover.description",
    },
    {
      key: "titlebar",
      labelKey: "themeEditor.colors.titlebar.label",
      token: "titlebar",
      descriptionKey: "themeEditor.colors.titlebar.description",
    },
  ],
} as const satisfies Record<string, readonly ThemeColorItem[]>

export const TERMINAL_GROUPS: Array<{
  titleKey: string
  items: TerminalColorItem[]
}> = [
  {
    titleKey: "themeEditor.terminalGroups.surface",
    items: [
      { key: "background", labelKey: "themeEditor.terminalColors.background" },
      { key: "foreground", labelKey: "themeEditor.terminalColors.foreground" },
      { key: "cursor", labelKey: "themeEditor.terminalColors.cursor" },
      { key: "selectionBackground", labelKey: "themeEditor.terminalColors.selectionBackground" },
    ],
  },
  {
    titleKey: "themeEditor.terminalGroups.ansi",
    items: [
      { key: "black", labelKey: "themeEditor.terminalColors.black" },
      { key: "red", labelKey: "themeEditor.terminalColors.red" },
      { key: "green", labelKey: "themeEditor.terminalColors.green" },
      { key: "yellow", labelKey: "themeEditor.terminalColors.yellow" },
      { key: "blue", labelKey: "themeEditor.terminalColors.blue" },
      { key: "magenta", labelKey: "themeEditor.terminalColors.magenta" },
      { key: "cyan", labelKey: "themeEditor.terminalColors.cyan" },
      { key: "white", labelKey: "themeEditor.terminalColors.white" },
    ],
  },
  {
    titleKey: "themeEditor.terminalGroups.brightAnsi",
    items: [
      { key: "brightBlack", labelKey: "themeEditor.terminalColors.brightBlack" },
      { key: "brightRed", labelKey: "themeEditor.terminalColors.brightRed" },
      { key: "brightGreen", labelKey: "themeEditor.terminalColors.brightGreen" },
      { key: "brightYellow", labelKey: "themeEditor.terminalColors.brightYellow" },
      { key: "brightBlue", labelKey: "themeEditor.terminalColors.brightBlue" },
      { key: "brightMagenta", labelKey: "themeEditor.terminalColors.brightMagenta" },
      { key: "brightCyan", labelKey: "themeEditor.terminalColors.brightCyan" },
      { key: "brightWhite", labelKey: "themeEditor.terminalColors.brightWhite" },
    ],
  },
]

export const TERMINAL_PLACEHOLDER = "#000000 / rgba(...) / hsl(...)"

export function terminalColorKey(key: keyof TerminalPalette): string {
  return `terminal-${key}`
}
