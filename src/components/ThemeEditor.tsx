import React, { useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"
import { Palette, Sparkles, Save, X } from "lucide-react"

import { TerminalPalettePreview } from "@/components/TerminalPalettePreview"
import { ThemePreviewSwatches } from "@/components/ThemePreviewSwatches"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useTheme } from "@/contexts/ThemeContext"
import { generateTerminalPaletteFromColors } from "@/lib/terminalPalette"
import type { CustomTheme, PresetThemeId, TerminalPalette, ThemeColors } from "@/types/theme"
import { PRESET_THEME_IDS } from "@/types/theme"

interface ThemeEditorProps {
  themeId?: string
  baseThemeId?: string
  onClose: () => void
  onSave?: (theme: CustomTheme) => void
}

type ThemeColorItem = {
  key: keyof ThemeColors
  label: string
  token: string
  description: string
}

const COLOR_GROUPS = {
  basic: [
    {
      key: "background",
      label: "Window background",
      token: "background",
      description: "Main app background and large surface color",
    },
    {
      key: "foreground",
      label: "Primary text",
      token: "foreground",
      description: "Default body text, titles and icons",
    },
    {
      key: "border",
      label: "Borders",
      token: "border",
      description: "Panel borders, dividers and subtle outlines",
    },
  ],
  card: [
    {
      key: "card",
      label: "Card surface",
      token: "card",
      description: "Settings panels, dialogs and cards",
    },
    {
      key: "cardForeground",
      label: "Card text",
      token: "cardForeground",
      description: "Main text inside card surfaces",
    },
    {
      key: "popover",
      label: "Popover surface",
      token: "popover",
      description: "Menus, color pickers and floating panels",
    },
    {
      key: "popoverForeground",
      label: "Popover text",
      token: "popoverForeground",
      description: "Text and icons inside floating panels",
    },
  ],
  primary: [
    {
      key: "primary",
      label: "Primary button",
      token: "primary",
      description: "Main actions, selected states and brand accent",
    },
    {
      key: "primaryForeground",
      label: "Primary text",
      token: "primaryForeground",
      description: "Text displayed on primary color",
    },
  ],
  secondary: [
    {
      key: "secondary",
      label: "Secondary button",
      token: "secondary",
      description: "Secondary actions and quiet surfaces",
    },
    {
      key: "secondaryForeground",
      label: "Secondary text",
      token: "secondaryForeground",
      description: "Text displayed on secondary surfaces",
    },
  ],
  accent: [
    {
      key: "accent",
      label: "Hover highlight",
      token: "accent",
      description: "List hover, menu hover and soft highlights",
    },
    {
      key: "accentForeground",
      label: "Highlight text",
      token: "accentForeground",
      description: "Text displayed on highlight surfaces",
    },
  ],
  muted: [
    {
      key: "muted",
      label: "Muted surface",
      token: "muted",
      description: "Disabled, quiet and low-emphasis areas",
    },
    {
      key: "mutedForeground",
      label: "Secondary text",
      token: "mutedForeground",
      description: "Hints, placeholders and helper text",
    },
  ],
  status: [
    {
      key: "destructive",
      label: "Danger",
      token: "destructive",
      description: "Delete, error and destructive actions",
    },
    {
      key: "destructiveForeground",
      label: "Danger text",
      token: "destructiveForeground",
      description: "Text displayed on danger color",
    },
    {
      key: "success",
      label: "Success",
      token: "success",
      description: "Success, connected and completed states",
    },
    {
      key: "successForeground",
      label: "Success text",
      token: "successForeground",
      description: "Text displayed on success color",
    },
    {
      key: "warning",
      label: "Warning",
      token: "warning",
      description: "Warnings, pending states and notices",
    },
    {
      key: "warningForeground",
      label: "Warning text",
      token: "warningForeground",
      description: "Text displayed on warning color",
    },
  ],
  input: [
    {
      key: "input",
      label: "Input border",
      token: "input",
      description: "Input, select and form control outlines",
    },
    {
      key: "ring",
      label: "Focus ring",
      token: "ring",
      description: "Keyboard focus and selection outline",
    },
  ],
  tabs: [
    {
      key: "tabBackground",
      label: "Tab bar",
      token: "tabBackground",
      description: "Top tab bar and terminal tab area",
    },
    {
      key: "tabActive",
      label: "Active tab",
      token: "tabActive",
      description: "Background of the active tab",
    },
    {
      key: "tabHover",
      label: "Tab hover",
      token: "tabHover",
      description: "Background when hovering a tab",
    },
    {
      key: "titlebar",
      label: "Titlebar",
      token: "titlebar",
      description: "Window titlebar and top chrome",
    },
  ],
} as const satisfies Record<string, readonly ThemeColorItem[]>

const TERMINAL_GROUPS: Array<{
  title: string
  items: Array<{ key: keyof TerminalPalette; label: string }>
}> = [
  {
    title: "Surface",
    items: [
      { key: "background", label: "Terminal Background" },
      { key: "foreground", label: "Terminal Foreground" },
      { key: "cursor", label: "Cursor" },
      { key: "selectionBackground", label: "Selection" },
    ],
  },
  {
    title: "ANSI",
    items: [
      { key: "black", label: "Black" },
      { key: "red", label: "Red" },
      { key: "green", label: "Green" },
      { key: "yellow", label: "Yellow" },
      { key: "blue", label: "Blue" },
      { key: "magenta", label: "Magenta" },
      { key: "cyan", label: "Cyan" },
      { key: "white", label: "White" },
    ],
  },
  {
    title: "Bright ANSI",
    items: [
      { key: "brightBlack", label: "Bright Black" },
      { key: "brightRed", label: "Bright Red" },
      { key: "brightGreen", label: "Bright Green" },
      { key: "brightYellow", label: "Bright Yellow" },
      { key: "brightBlue", label: "Bright Blue" },
      { key: "brightMagenta", label: "Bright Magenta" },
      { key: "brightCyan", label: "Bright Cyan" },
      { key: "brightWhite", label: "Bright White" },
    ],
  },
]

function isHslValue(value: string): boolean {
  return /^\d+(\.\d+)?\s+\d+(\.\d+)?%\s+\d+(\.\d+)?%$/.test(value.trim())
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function componentToHex(value: number): string {
  return Math.round(clamp(value, 0, 255))
    .toString(16)
    .padStart(2, "0")
}

function rgbToHex(r: number, g: number, b: number): string {
  return `#${componentToHex(r)}${componentToHex(g)}${componentToHex(b)}`
}

function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  const normalized = hex.trim().replace(/^#/, "")
  const expanded =
    normalized.length === 3
      ? normalized
          .split("")
          .map((char) => char + char)
          .join("")
      : normalized

  if (!/^[\da-f]{6}$/i.test(expanded)) {
    return null
  }

  return {
    r: parseInt(expanded.slice(0, 2), 16),
    g: parseInt(expanded.slice(2, 4), 16),
    b: parseInt(expanded.slice(4, 6), 16),
  }
}

function hslToRgb(h: number, s: number, l: number): { r: number; g: number; b: number } {
  const hue = (((h % 360) + 360) % 360) / 360
  const saturation = clamp(s, 0, 100) / 100
  const lightness = clamp(l, 0, 100) / 100

  if (saturation === 0) {
    const gray = lightness * 255
    return { r: gray, g: gray, b: gray }
  }

  const q =
    lightness < 0.5 ? lightness * (1 + saturation) : lightness + saturation - lightness * saturation
  const p = 2 * lightness - q
  const hueToRgb = (t: number) => {
    let shifted = t
    if (shifted < 0) shifted += 1
    if (shifted > 1) shifted -= 1
    if (shifted < 1 / 6) return p + (q - p) * 6 * shifted
    if (shifted < 1 / 2) return q
    if (shifted < 2 / 3) return p + (q - p) * (2 / 3 - shifted) * 6
    return p
  }

  return {
    r: hueToRgb(hue + 1 / 3) * 255,
    g: hueToRgb(hue) * 255,
    b: hueToRgb(hue - 1 / 3) * 255,
  }
}

function rgbToHslToken(r: number, g: number, b: number): string {
  const red = clamp(r, 0, 255) / 255
  const green = clamp(g, 0, 255) / 255
  const blue = clamp(b, 0, 255) / 255
  const max = Math.max(red, green, blue)
  const min = Math.min(red, green, blue)
  const lightness = (max + min) / 2

  if (max === min) {
    return `0 0% ${Math.round(lightness * 100)}%`
  }

  const delta = max - min
  const saturation = lightness > 0.5 ? delta / (2 - max - min) : delta / (max + min)
  let hue = 0

  if (max === red) {
    hue = (green - blue) / delta + (green < blue ? 6 : 0)
  } else if (max === green) {
    hue = (blue - red) / delta + 2
  } else {
    hue = (red - green) / delta + 4
  }

  return `${Math.round(hue * 60)} ${Math.round(saturation * 100)}% ${Math.round(lightness * 100)}%`
}

function hexToHslToken(hex: string): string | null {
  const rgb = hexToRgb(hex)
  return rgb ? rgbToHslToken(rgb.r, rgb.g, rgb.b) : null
}

function parseHslParts(value: string): { h: number; s: number; l: number } | null {
  const normalized = value.trim()
  const tokenMatch = normalized.match(/^(\d+(?:\.\d+)?)\s+(\d+(?:\.\d+)?)%\s+(\d+(?:\.\d+)?)%$/)
  const functionMatch = normalized.match(
    /^hsla?\(\s*(\d+(?:\.\d+)?)(?:deg)?[\s,]+(\d+(?:\.\d+)?)%[\s,]+(\d+(?:\.\d+)?)%/i
  )
  const match = tokenMatch ?? functionMatch

  if (!match) return null

  return {
    h: Number(match[1]),
    s: Number(match[2]),
    l: Number(match[3]),
  }
}

function colorToHex(value: string): string | null {
  const normalized = value.trim()

  if (/^#[\da-f]{3}$/i.test(normalized) || /^#[\da-f]{6}$/i.test(normalized)) {
    const rgb = hexToRgb(normalized)
    return rgb ? rgbToHex(rgb.r, rgb.g, rgb.b) : null
  }

  const rgbMatch = normalized.match(
    /^rgba?\(\s*(\d+(?:\.\d+)?)[\s,]+(\d+(?:\.\d+)?)[\s,]+(\d+(?:\.\d+)?)/i
  )
  if (rgbMatch) {
    return rgbToHex(Number(rgbMatch[1]), Number(rgbMatch[2]), Number(rgbMatch[3]))
  }

  const hsl = parseHslParts(normalized)
  if (hsl) {
    const rgb = hslToRgb(hsl.h, hsl.s, hsl.l)
    return rgbToHex(rgb.r, rgb.g, rgb.b)
  }

  return null
}

function rgbToHsl(r: number, g: number, b: number): { h: number; s: number; l: number } {
  const red = clamp(r, 0, 255) / 255
  const green = clamp(g, 0, 255) / 255
  const blue = clamp(b, 0, 255) / 255
  const max = Math.max(red, green, blue)
  const min = Math.min(red, green, blue)
  const lightness = (max + min) / 2

  if (max === min) {
    return { h: 0, s: 0, l: Math.round(lightness * 100) }
  }

  const delta = max - min
  const saturation = lightness > 0.5 ? delta / (2 - max - min) : delta / (max + min)
  let hue = 0

  if (max === red) {
    hue = (green - blue) / delta + (green < blue ? 6 : 0)
  } else if (max === green) {
    hue = (blue - red) / delta + 2
  } else {
    hue = (red - green) / delta + 4
  }

  return {
    h: Math.round(hue * 60),
    s: Math.round(saturation * 100),
    l: Math.round(lightness * 100),
  }
}

function hexToHslParts(hex: string): { h: number; s: number; l: number } | null {
  const rgb = hexToRgb(hex)
  return rgb ? rgbToHsl(rgb.r, rgb.g, rgb.b) : null
}

function readableTextHex(backgroundHex: string): string {
  const rgb = hexToRgb(backgroundHex)
  if (!rgb) return "#ffffff"
  const luminance = (0.299 * rgb.r + 0.587 * rgb.g + 0.114 * rgb.b) / 255
  return luminance > 0.58 ? "#111827" : "#ffffff"
}

function relativeLuminance(hex: string): number | null {
  const rgb = hexToRgb(hex)
  if (!rgb) return null
  const channel = (value: number) => {
    const normalized = value / 255
    return normalized <= 0.03928 ? normalized / 12.92 : Math.pow((normalized + 0.055) / 1.055, 2.4)
  }
  return 0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b)
}

function contrastRatio(first: string, second: string): number | null {
  const firstLum = relativeLuminance(first)
  const secondLum = relativeLuminance(second)
  if (firstLum === null || secondLum === null) return null
  const lighter = Math.max(firstLum, secondLum)
  const darker = Math.min(firstLum, secondLum)
  return (lighter + 0.05) / (darker + 0.05)
}

function resolveCssColor(value: string): string {
  return normalizeColorPreview(value)
}

function createThemeSuggestions(colors: ThemeColors): string[] {
  const values = [
    colors.primary,
    colors.accent,
    colors.success,
    colors.warning,
    colors.destructive,
    colors.background,
    colors.foreground,
    colors.card,
    colors.muted,
    colors.border,
  ]

  return Array.from(
    new Set(values.map(colorToHex).filter((value): value is string => Boolean(value)))
  )
}

function createTerminalSuggestions(colors: ThemeColors, terminal: TerminalPalette): string[] {
  const values = [
    terminal.background,
    terminal.foreground,
    terminal.blue,
    terminal.green,
    terminal.red,
    terminal.yellow,
    terminal.magenta,
    terminal.cyan,
    ...createThemeSuggestions(colors),
  ]

  return Array.from(
    new Set(values.map(colorToHex).filter((value): value is string => Boolean(value)))
  )
}

function normalizeColorPreview(value: string): string {
  const normalized = value.trim()

  if (
    normalized.startsWith("#") ||
    normalized.startsWith("rgb(") ||
    normalized.startsWith("rgba(") ||
    normalized.startsWith("hsl(") ||
    normalized.startsWith("hsla(")
  ) {
    return normalized
  }

  if (isHslValue(normalized)) {
    return `hsl(${normalized})`
  }

  return normalized
}

function isPresetThemeId(themeId: string): themeId is PresetThemeId {
  return PRESET_THEME_IDS.includes(themeId as PresetThemeId)
}

export const ThemeEditor: React.FC<ThemeEditorProps> = ({
  themeId,
  baseThemeId,
  onClose,
  onSave,
}) => {
  const { t } = useTranslation()
  const { getTheme, createCustomTheme, presetThemeOverrides, updateCustomTheme } = useTheme()

  const [name, setName] = useState("")
  const [description, setDescription] = useState("")
  const [colors, setColors] = useState<ThemeColors | null>(null)
  const [terminal, setTerminal] = useState<TerminalPalette | null>(null)
  const [activeColorKey, setActiveColorKey] = useState<string | null>(null)
  const [isSaving, setIsSaving] = useState(false)

  useEffect(() => {
    const initializeTheme = async () => {
      if (themeId) {
        const presetOverride = presetThemeOverrides.find((theme) => theme.id === themeId)
        const theme = getTheme(themeId)
        if (theme) {
          const resolvedName =
            presetOverride?.name || (isPresetThemeId(themeId) ? t(`theme.${themeId}`) : theme.name)
          const resolvedDescription =
            presetOverride?.description ||
            (isPresetThemeId(themeId) ? t(`theme.${themeId}Desc`) : (theme.description ?? ""))

          setName(resolvedName)
          setDescription(resolvedDescription)
          setColors({ ...theme.colors })
          setTerminal({ ...theme.terminal })
        }
      } else if (baseThemeId) {
        const themeUtils = await import("@/lib/themeUtils")
        const themeData = themeUtils.createCustomThemeFromPreset(
          baseThemeId as PresetThemeId,
          `Custom ${baseThemeId}`,
          `Based on ${baseThemeId}`
        )
        setName(themeData.name)
        setDescription(themeData.description || "")
        setColors(themeData.colors)
        setTerminal(themeData.terminal)
      }
    }

    initializeTheme()
  }, [themeId, baseThemeId, getTheme, presetThemeOverrides, t])

  const handleColorChange = (key: keyof ThemeColors, value: string) => {
    if (colors) {
      setColors({ ...colors, [key]: value })
    }
  }

  const handleTerminalChange = (key: keyof TerminalPalette, value: string) => {
    if (terminal) {
      setTerminal({ ...terminal, [key]: value })
    }
  }

  const handleGenerateTerminalPalette = () => {
    if (!colors) return
    setTerminal(generateTerminalPaletteFromColors(colors))
  }

  const handleSave = async () => {
    if (!colors || !terminal || !name.trim()) return

    setIsSaving(true)
    try {
      if (themeId) {
        await updateCustomTheme(themeId, {
          name: name.trim(),
          description: description.trim(),
          colors,
          terminal,
        })
      } else {
        const newTheme = await createCustomTheme({
          name: name.trim(),
          description: description.trim(),
          colors,
          terminal,
          baseTheme: baseThemeId,
          isCustom: true,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        })
        onSave?.(newTheme)
      }
      onClose()
    } catch (error) {
      console.error("Failed to save theme:", error)
    } finally {
      setIsSaving(false)
    }
  }

  if (!colors || !terminal) {
    return null
  }

  const themeSuggestions = createThemeSuggestions(colors)
  const terminalSuggestions = createTerminalSuggestions(colors, terminal)

  const renderThemeColorInput = (item: ThemeColorItem) => (
    <ColorInput
      key={item.key}
      label={item.label}
      value={colors[item.key]}
      onChange={(value) => handleColorChange(item.key, value)}
      suggestions={themeSuggestions}
      token={item.token}
      description={item.description}
      isOpen={activeColorKey === item.key}
      onOpenChange={(open) => setActiveColorKey(open ? item.key : null)}
    />
  )

  const renderTerminalColorInput = (item: { key: keyof TerminalPalette; label: string }) => (
    <TerminalColorInput
      key={item.key}
      label={item.label}
      value={terminal[item.key]}
      onChange={(value) => handleTerminalChange(item.key, value)}
      suggestions={terminalSuggestions}
      isOpen={activeColorKey === `terminal-${item.key}`}
      onOpenChange={(open) => setActiveColorKey(open ? `terminal-${item.key}` : null)}
    />
  )

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="flex max-h-[92vh] w-[min(78rem,calc(100vw-2rem))] flex-col overflow-hidden sm:max-w-6xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Palette size={16} />
            {themeId ? t("themeEditor.editTheme") : t("themeEditor.createTheme")}
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 space-y-4 overflow-y-auto py-2">
          <div className="space-y-3">
            <div>
              <Label htmlFor="theme-name">{t("themeEditor.name")}</Label>
              <Input
                id="theme-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t("themeEditor.namePlaceholder")}
              />
            </div>
            <div>
              <Label htmlFor="theme-description">{t("themeEditor.description")}</Label>
              <Input
                id="theme-description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t("themeEditor.descriptionPlaceholder")}
              />
            </div>
          </div>

          <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
            <Tabs defaultValue="basic" className="w-full min-w-0">
              <TabsList className="grid h-auto w-full grid-cols-2 gap-1 sm:grid-cols-5">
                <TabsTrigger
                  value="basic"
                  className="min-w-0 px-2 text-xs whitespace-normal sm:text-sm"
                >
                  {t("themeEditor.basic")}
                </TabsTrigger>
                <TabsTrigger
                  value="components"
                  className="min-w-0 px-2 text-xs whitespace-normal sm:text-sm"
                >
                  {t("themeEditor.components")}
                </TabsTrigger>
                <TabsTrigger
                  value="status"
                  className="min-w-0 px-2 text-xs whitespace-normal sm:text-sm"
                >
                  {t("themeEditor.status")}
                </TabsTrigger>
                <TabsTrigger
                  value="tabs"
                  className="min-w-0 px-2 text-xs whitespace-normal sm:text-sm"
                >
                  {t("themeEditor.tabs")}
                </TabsTrigger>
                <TabsTrigger
                  value="terminal"
                  className="min-w-0 px-2 text-xs whitespace-normal sm:text-sm"
                >
                  {t("themeEditor.terminal")}
                </TabsTrigger>
              </TabsList>

              <TabsContent value="basic" className="mt-4 space-y-3">
                {COLOR_GROUPS.basic.map(renderThemeColorInput)}
                {COLOR_GROUPS.input.map(renderThemeColorInput)}
              </TabsContent>

              <TabsContent value="components" className="mt-4 space-y-3">
                <div className="space-y-2">
                  <h4 className="text-sm font-medium">Card</h4>
                  {COLOR_GROUPS.card.map(renderThemeColorInput)}
                </div>
                <div className="space-y-2">
                  <h4 className="text-sm font-medium">Primary</h4>
                  {COLOR_GROUPS.primary.map(renderThemeColorInput)}
                </div>
                <div className="space-y-2">
                  <h4 className="text-sm font-medium">Secondary</h4>
                  {COLOR_GROUPS.secondary.map(renderThemeColorInput)}
                </div>
                <div className="space-y-2">
                  <h4 className="text-sm font-medium">Accent & Muted</h4>
                  {[...COLOR_GROUPS.accent, ...COLOR_GROUPS.muted].map(renderThemeColorInput)}
                </div>
              </TabsContent>

              <TabsContent value="status" className="mt-4 space-y-3">
                {COLOR_GROUPS.status.map(renderThemeColorInput)}
              </TabsContent>

              <TabsContent value="tabs" className="mt-4 space-y-3">
                {COLOR_GROUPS.tabs.map(renderThemeColorInput)}
              </TabsContent>

              <TabsContent value="terminal" className="mt-4 space-y-5">
                <div className="space-y-2">
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <h4 className="text-sm font-medium">{t("common.preview")}</h4>
                      <p className="text-muted-foreground text-xs">
                        {t("themeEditor.terminalPreviewDesc")}
                      </p>
                    </div>
                    <ThemePreviewSwatches compact palette={terminal} />
                  </div>

                  <div className="bg-muted/20 border-border overflow-hidden rounded-lg border">
                    <TerminalPalettePreview palette={terminal} />
                  </div>
                </div>

                <div className="bg-muted/40 border-border flex flex-wrap items-center justify-between gap-3 rounded-lg border px-4 py-3">
                  <div className="space-y-1">
                    <p className="text-sm font-medium">{t("themeEditor.terminalAutoGenerate")}</p>
                    <p className="text-muted-foreground text-xs">
                      {t("themeEditor.terminalAutoGenerateDesc")}
                    </p>
                  </div>
                  <Button type="button" variant="outline" onClick={handleGenerateTerminalPalette}>
                    <Sparkles size={14} className="mr-2" />
                    {t("themeEditor.generatePalette")}
                  </Button>
                </div>
                {TERMINAL_GROUPS.map((group) => (
                  <div key={group.title} className="space-y-3">
                    <h4 className="text-sm font-medium">{group.title}</h4>
                    {group.items.map(renderTerminalColorInput)}
                  </div>
                ))}
              </TabsContent>
            </Tabs>

            <ThemeLivePreview colors={colors} terminal={terminal} />
          </div>
        </div>

        <div className="flex justify-end gap-2 border-t pt-4">
          <Button variant="outline" onClick={onClose}>
            <X size={16} className="mr-2" />
            {t("common.cancel")}
          </Button>
          <Button onClick={handleSave} disabled={isSaving || !name.trim()}>
            <Save size={16} className="mr-2" />
            {isSaving ? t("common.saving") : t("common.save")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

interface ColorInputProps {
  label: string
  value: string
  onChange: (value: string) => void
  colorMode?: "hsl-token" | "css"
  placeholder?: string
  suggestions?: string[]
  token?: string
  description?: string
  isOpen?: boolean
  onOpenChange?: (open: boolean) => void
}

const ColorInput: React.FC<ColorInputProps> = ({
  label,
  value,
  onChange,
  colorMode = "hsl-token",
  placeholder = "0 0% 0%",
  suggestions = [],
  token,
  description,
  isOpen = false,
  onOpenChange,
}) => {
  const pickerValue = colorToHex(value) ?? "#000000"
  const hslParts = hexToHslParts(pickerValue) ?? { h: 0, s: 0, l: 0 }
  const previewText = readableTextHex(pickerValue)

  const applyHex = (nextHex: string) => {
    if (colorMode === "hsl-token") {
      const hslToken = hexToHslToken(nextHex)
      if (hslToken) onChange(hslToken)
      return
    }

    onChange(nextHex)
  }

  const applyHsl = (updates: Partial<{ h: number; s: number; l: number }>) => {
    const next = { ...hslParts, ...updates }
    const rgb = hslToRgb(next.h, next.s, next.l)
    applyHex(rgbToHex(rgb.r, rgb.g, rgb.b))
  }

  return (
    <div className="hover:border-border/80 hover:bg-muted/20 relative rounded-lg border border-transparent p-2 transition-colors">
      <div className="flex items-center gap-3">
        <button
          type="button"
          aria-label={`Open ${label} color picker`}
          onClick={() => onOpenChange?.(!isOpen)}
          className="border-border ring-offset-background focus-visible:ring-ring relative size-10 shrink-0 overflow-hidden rounded-lg border shadow-sm transition-transform hover:scale-105 focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
          style={{ backgroundColor: normalizeColorPreview(value) }}
        >
          <span className="absolute inset-x-0 bottom-0 h-2 bg-black/15" />
        </button>
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center justify-between gap-2">
            <div className="min-w-0">
              <Label className="text-xs font-medium">{label}</Label>
              {token && <p className="text-muted-foreground font-mono text-[10px]">{token}</p>}
            </div>
            <span className="text-muted-foreground font-mono text-[10px] uppercase">
              {pickerValue}
            </span>
          </div>
          {description && <p className="text-muted-foreground mb-2 text-xs">{description}</p>}
          <Input
            value={value}
            onFocus={() => onOpenChange?.(true)}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            className="h-8 font-mono text-sm"
          />
        </div>
      </div>

      {isOpen && (
        <div className="bg-popover text-popover-foreground border-border absolute top-full right-0 z-50 mt-2 w-[min(20rem,calc(100vw-3rem))] rounded-xl border p-3 shadow-2xl">
          <SaturationLightnessPicker
            hsl={hslParts}
            previewHex={pickerValue}
            previewText={previewText}
            onChange={(updates) => applyHsl(updates)}
          />

          <div className="mt-3 space-y-3">
            <ColorSlider
              label="Hue"
              value={hslParts.h}
              min={0}
              max={360}
              background="linear-gradient(90deg, #ef4444, #f59e0b, #eab308, #22c55e, #06b6d4, #3b82f6, #a855f7, #ef4444)"
              onChange={(next) => applyHsl({ h: next })}
            />
          </div>

          <div className="mt-3 grid grid-cols-[1fr_auto] gap-2">
            <Input
              value={pickerValue}
              onChange={(e) => applyHex(e.target.value)}
              className="h-8 font-mono text-sm"
            />
            <input
              aria-label={`Use system picker for ${label}`}
              type="color"
              value={pickerValue}
              onChange={(e) => applyHex(e.target.value)}
              className="border-input bg-background h-8 w-10 cursor-pointer rounded-md border p-1"
            />
          </div>

          {suggestions.length > 0 && (
            <div className="mt-3">
              <p className="text-muted-foreground mb-2 text-[11px] font-medium">Theme colors</p>
              <div className="grid grid-cols-10 gap-1.5">
                {suggestions.slice(0, 20).map((suggestion) => (
                  <button
                    key={suggestion}
                    type="button"
                    aria-label={`Use ${suggestion}`}
                    onClick={() => applyHex(suggestion)}
                    className="border-border size-5 rounded border shadow-sm transition-transform hover:scale-110"
                    style={{ backgroundColor: suggestion }}
                  />
                ))}
              </div>
            </div>
          )}

          <div className="mt-3 flex items-center justify-between gap-2 border-t pt-3">
            <div className="text-muted-foreground text-[11px]">
              Text on this color: <span className="font-mono">{previewText}</span>
            </div>
            <Button type="button" variant="outline" size="sm" onClick={() => onOpenChange?.(false)}>
              Done
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}

interface SaturationLightnessPickerProps {
  hsl: { h: number; s: number; l: number }
  previewHex: string
  previewText: string
  onChange: (updates: Partial<{ s: number; l: number }>) => void
}

const SaturationLightnessPicker: React.FC<SaturationLightnessPickerProps> = ({
  hsl,
  previewHex,
  previewText,
  onChange,
}) => {
  const pickerRef = useRef<HTMLDivElement | null>(null)
  const [isDragging, setIsDragging] = useState(false)

  const updateFromPointer = (clientX: number, clientY: number) => {
    const rect = pickerRef.current?.getBoundingClientRect()
    if (!rect) return

    const x = clamp((clientX - rect.left) / rect.width, 0, 1)
    const y = clamp((clientY - rect.top) / rect.height, 0, 1)
    onChange({ s: Math.round(x * 100), l: Math.round((1 - y) * 100) })
  }

  const handlePointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    setIsDragging(true)
    updateFromPointer(event.clientX, event.clientY)
  }

  const handlePointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!isDragging) return
    updateFromPointer(event.clientX, event.clientY)
  }

  const stopDragging = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
    setIsDragging(false)
  }

  return (
    <div>
      <div
        ref={pickerRef}
        role="slider"
        tabIndex={0}
        aria-label="Choose saturation and lightness"
        aria-valuetext={`Saturation ${Math.round(hsl.s)}%, lightness ${Math.round(hsl.l)}%`}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={stopDragging}
        onPointerCancel={stopDragging}
        className="border-border relative h-32 touch-none overflow-hidden rounded-lg border shadow-inner"
        style={{ backgroundColor: `hsl(${hsl.h} 100% 50%)` }}
      >
        <div className="absolute inset-0 bg-gradient-to-r from-white to-transparent" />
        <div className="absolute inset-0 bg-gradient-to-t from-black to-transparent" />
        <div
          className="absolute size-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgba(0,0,0,0.65),0_4px_12px_rgba(0,0,0,0.35)]"
          style={{ left: `${hsl.s}%`, top: `${100 - hsl.l}%`, backgroundColor: previewHex }}
        />
        <div
          className="absolute bottom-3 left-3 rounded-md px-2 py-1 text-xs font-medium shadow"
          style={{ backgroundColor: previewHex, color: previewText }}
        >
          {previewHex}
        </div>
      </div>
      <p className="text-muted-foreground mt-2 text-[11px]">
        Click or drag the panel to adjust saturation and lightness; use the Hue slider below to
        change hue.
      </p>
    </div>
  )
}

interface ColorSliderProps {
  label: string
  value: number
  min: number
  max: number
  background: string
  onChange: (value: number) => void
}

const ColorSlider: React.FC<ColorSliderProps> = ({
  label,
  value,
  min,
  max,
  background,
  onChange,
}) => {
  return (
    <label className="block space-y-1">
      <div className="flex items-center justify-between text-[11px]">
        <span className="text-muted-foreground font-medium">{label}</span>
        <span className="font-mono">{Math.round(value)}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        value={Math.round(value)}
        onChange={(e) => onChange(Number(e.target.value))}
        className="border-border accent-primary h-3 w-full cursor-pointer appearance-none rounded-full border bg-transparent"
        style={{ background }}
      />
    </label>
  )
}

const TerminalColorInput: React.FC<ColorInputProps> = ({
  label,
  value,
  onChange,
  suggestions,
  isOpen,
  onOpenChange,
}) => {
  return (
    <ColorInput
      label={label}
      value={value}
      onChange={onChange}
      colorMode="css"
      placeholder="#000000 / rgba(...) / hsl(...)"
      suggestions={suggestions}
      isOpen={isOpen}
      onOpenChange={onOpenChange}
    />
  )
}

interface ThemeLivePreviewProps {
  colors: ThemeColors
  terminal: TerminalPalette
}

const ThemeLivePreview: React.FC<ThemeLivePreviewProps> = ({ colors, terminal }) => {
  const contrastChecks = [
    {
      label: "Text",
      ratio: contrastRatio(
        colorToHex(colors.background) ?? "",
        colorToHex(colors.foreground) ?? ""
      ),
    },
    {
      label: "Primary",
      ratio: contrastRatio(
        colorToHex(colors.primary) ?? "",
        colorToHex(colors.primaryForeground) ?? ""
      ),
    },
    {
      label: "Card",
      ratio: contrastRatio(colorToHex(colors.card) ?? "", colorToHex(colors.cardForeground) ?? ""),
    },
  ]

  return (
    <aside className="xl:sticky xl:top-0 xl:self-start">
      <div className="border-border bg-card overflow-hidden rounded-2xl border shadow-sm">
        <div className="border-border border-b px-4 py-3">
          <p className="text-sm font-semibold">Live preview</p>
          <p className="text-muted-foreground text-xs">
            UI, tabs, status and terminal in one place.
          </p>
        </div>

        <div
          className="space-y-4 p-4"
          style={{
            background: resolveCssColor(colors.background),
            color: resolveCssColor(colors.foreground),
          }}
        >
          <div
            className="overflow-hidden rounded-xl border"
            style={{
              borderColor: resolveCssColor(colors.border),
              background: resolveCssColor(colors.card),
              color: resolveCssColor(colors.cardForeground),
            }}
          >
            <div
              className="flex items-center justify-between px-3 py-2"
              style={{ background: resolveCssColor(colors.titlebar) }}
            >
              <div className="flex gap-1.5">
                <span className="size-2.5 rounded-full bg-red-400" />
                <span className="size-2.5 rounded-full bg-yellow-400" />
                <span className="size-2.5 rounded-full bg-green-400" />
              </div>
              <span className="text-[11px] font-medium">tTerm Theme Lab</span>
            </div>
            <div
              className="flex gap-1 border-y px-2 py-2"
              style={{
                borderColor: resolveCssColor(colors.border),
                background: resolveCssColor(colors.tabBackground),
              }}
            >
              <span
                className="rounded-md px-2 py-1 text-xs"
                style={{
                  background: resolveCssColor(colors.tabActive),
                  color: resolveCssColor(colors.foreground),
                }}
              >
                Local
              </span>
              <span
                className="rounded-md px-2 py-1 text-xs"
                style={{
                  background: resolveCssColor(colors.tabHover),
                  color: resolveCssColor(colors.mutedForeground),
                }}
              >
                SSH
              </span>
            </div>
            <div className="space-y-3 p-3">
              <div
                className="rounded-lg border p-3"
                style={{
                  borderColor: resolveCssColor(colors.border),
                  background: resolveCssColor(colors.background),
                }}
              >
                <p className="text-sm font-semibold">Connection settings</p>
                <p
                  className="mt-1 text-xs"
                  style={{ color: resolveCssColor(colors.mutedForeground) }}
                >
                  Preview how surfaces, labels and muted text sit together.
                </p>
                <div className="mt-3 flex gap-2">
                  <button
                    type="button"
                    className="rounded-md px-3 py-1.5 text-xs font-medium"
                    style={{
                      background: resolveCssColor(colors.primary),
                      color: resolveCssColor(colors.primaryForeground),
                    }}
                  >
                    Connect
                  </button>
                  <button
                    type="button"
                    className="rounded-md border px-3 py-1.5 text-xs"
                    style={{
                      borderColor: resolveCssColor(colors.border),
                      background: resolveCssColor(colors.secondary),
                      color: resolveCssColor(colors.secondaryForeground),
                    }}
                  >
                    Cancel
                  </button>
                </div>
              </div>

              <div className="grid grid-cols-3 gap-2 text-center text-[11px]">
                <PreviewBadge
                  label="Success"
                  background={colors.success}
                  foreground={colors.successForeground}
                />
                <PreviewBadge
                  label="Warning"
                  background={colors.warning}
                  foreground={colors.warningForeground}
                />
                <PreviewBadge
                  label="Danger"
                  background={colors.destructive}
                  foreground={colors.destructiveForeground}
                />
              </div>
            </div>
          </div>

          <div
            className="overflow-hidden rounded-xl border font-mono text-xs"
            style={{
              borderColor: resolveCssColor(colors.border),
              background: normalizeColorPreview(terminal.background),
              color: normalizeColorPreview(terminal.foreground),
            }}
          >
            <div
              className="border-b px-3 py-2"
              style={{
                borderColor: resolveCssColor(colors.border),
                color: normalizeColorPreview(terminal.cursor),
              }}
            >
              ~/theme-lab
            </div>
            <div className="space-y-1 p-3">
              <p>
                <span style={{ color: normalizeColorPreview(terminal.green) }}>stone@tterm</span>:
                <span style={{ color: normalizeColorPreview(terminal.blue) }}>~/Code</span>$ pnpm
                dev
              </p>
              <p>
                <span style={{ color: normalizeColorPreview(terminal.yellow) }}>vite</span> ready in{" "}
                <span style={{ color: normalizeColorPreview(terminal.cyan) }}>312ms</span>
              </p>
              <p style={{ color: normalizeColorPreview(terminal.magenta) }}>
                theme preview updated
              </p>
            </div>
          </div>

          <div className="grid grid-cols-3 gap-2">
            {contrastChecks.map((check) => (
              <div
                key={check.label}
                className="rounded-lg border px-2 py-2 text-center"
                style={{
                  borderColor: resolveCssColor(colors.border),
                  background: resolveCssColor(colors.card),
                }}
              >
                <p
                  className="text-[10px]"
                  style={{ color: resolveCssColor(colors.mutedForeground) }}
                >
                  {check.label}
                </p>
                <p className="text-xs font-semibold">
                  {check.ratio ? check.ratio.toFixed(1) : "--"}:1
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </aside>
  )
}

interface PreviewBadgeProps {
  label: string
  background: string
  foreground: string
}

const PreviewBadge: React.FC<PreviewBadgeProps> = ({ label, background, foreground }) => {
  return (
    <div
      className="rounded-md px-2 py-2 font-medium"
      style={{ background: resolveCssColor(background), color: resolveCssColor(foreground) }}
    >
      {label}
    </div>
  )
}
