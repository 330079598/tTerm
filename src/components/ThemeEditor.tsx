import React, { useEffect, useState } from "react"
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

const COLOR_GROUPS = {
  basic: [
    { key: "background", label: "Background" },
    { key: "foreground", label: "Foreground" },
    { key: "border", label: "Border" },
  ],
  card: [
    { key: "card", label: "Card" },
    { key: "cardForeground", label: "Card Foreground" },
  ],
  primary: [
    { key: "primary", label: "Primary" },
    { key: "primaryForeground", label: "Primary Foreground" },
  ],
  secondary: [
    { key: "secondary", label: "Secondary" },
    { key: "secondaryForeground", label: "Secondary Foreground" },
  ],
  accent: [
    { key: "accent", label: "Accent" },
    { key: "accentForeground", label: "Accent Foreground" },
  ],
  muted: [
    { key: "muted", label: "Muted" },
    { key: "mutedForeground", label: "Muted Foreground" },
  ],
  status: [
    { key: "destructive", label: "Destructive" },
    { key: "destructiveForeground", label: "Destructive Foreground" },
    { key: "success", label: "Success" },
    { key: "successForeground", label: "Success Foreground" },
    { key: "warning", label: "Warning" },
    { key: "warningForeground", label: "Warning Foreground" },
  ],
  input: [
    { key: "input", label: "Input" },
    { key: "ring", label: "Ring (Focus)" },
  ],
  tabs: [
    { key: "tabBackground", label: "Tab Background" },
    { key: "tabActive", label: "Tab Active" },
    { key: "tabHover", label: "Tab Hover" },
    { key: "titlebar", label: "Titlebar" },
  ],
} as const

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

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="flex max-h-[90vh] w-[min(64rem,calc(100vw-2rem))] flex-col overflow-hidden sm:max-w-4xl">
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

          <Tabs defaultValue="basic" className="w-full">
            <TabsList className="grid h-auto w-full grid-cols-2 gap-1 sm:grid-cols-5">
              <TabsTrigger value="basic" className="min-w-0 whitespace-normal px-2 text-xs sm:text-sm">
                {t("themeEditor.basic")}
              </TabsTrigger>
              <TabsTrigger
                value="components"
                className="min-w-0 whitespace-normal px-2 text-xs sm:text-sm"
              >
                {t("themeEditor.components")}
              </TabsTrigger>
              <TabsTrigger value="status" className="min-w-0 whitespace-normal px-2 text-xs sm:text-sm">
                {t("themeEditor.status")}
              </TabsTrigger>
              <TabsTrigger value="tabs" className="min-w-0 whitespace-normal px-2 text-xs sm:text-sm">
                {t("themeEditor.tabs")}
              </TabsTrigger>
              <TabsTrigger
                value="terminal"
                className="min-w-0 whitespace-normal px-2 text-xs sm:text-sm"
              >
                {t("themeEditor.terminal")}
              </TabsTrigger>
            </TabsList>

            <TabsContent value="basic" className="mt-4 space-y-3">
              {COLOR_GROUPS.basic.map((item) => (
                <ColorInput
                  key={item.key}
                  label={item.label}
                  value={colors[item.key as keyof ThemeColors]}
                  onChange={(value) => handleColorChange(item.key as keyof ThemeColors, value)}
                />
              ))}
              {COLOR_GROUPS.input.map((item) => (
                <ColorInput
                  key={item.key}
                  label={item.label}
                  value={colors[item.key as keyof ThemeColors]}
                  onChange={(value) => handleColorChange(item.key as keyof ThemeColors, value)}
                />
              ))}
            </TabsContent>

            <TabsContent value="components" className="mt-4 space-y-3">
              <div className="space-y-2">
                <h4 className="text-sm font-medium">Card</h4>
                {COLOR_GROUPS.card.map((item) => (
                  <ColorInput
                    key={item.key}
                    label={item.label}
                    value={colors[item.key as keyof ThemeColors]}
                    onChange={(value) => handleColorChange(item.key as keyof ThemeColors, value)}
                  />
                ))}
              </div>
              <div className="space-y-2">
                <h4 className="text-sm font-medium">Primary</h4>
                {COLOR_GROUPS.primary.map((item) => (
                  <ColorInput
                    key={item.key}
                    label={item.label}
                    value={colors[item.key as keyof ThemeColors]}
                    onChange={(value) => handleColorChange(item.key as keyof ThemeColors, value)}
                  />
                ))}
              </div>
              <div className="space-y-2">
                <h4 className="text-sm font-medium">Secondary</h4>
                {COLOR_GROUPS.secondary.map((item) => (
                  <ColorInput
                    key={item.key}
                    label={item.label}
                    value={colors[item.key as keyof ThemeColors]}
                    onChange={(value) => handleColorChange(item.key as keyof ThemeColors, value)}
                  />
                ))}
              </div>
              <div className="space-y-2">
                <h4 className="text-sm font-medium">Accent & Muted</h4>
                {[...COLOR_GROUPS.accent, ...COLOR_GROUPS.muted].map((item) => (
                  <ColorInput
                    key={item.key}
                    label={item.label}
                    value={colors[item.key as keyof ThemeColors]}
                    onChange={(value) => handleColorChange(item.key as keyof ThemeColors, value)}
                  />
                ))}
              </div>
            </TabsContent>

            <TabsContent value="status" className="mt-4 space-y-3">
              {COLOR_GROUPS.status.map((item) => (
                <ColorInput
                  key={item.key}
                  label={item.label}
                  value={colors[item.key as keyof ThemeColors]}
                  onChange={(value) => handleColorChange(item.key as keyof ThemeColors, value)}
                />
              ))}
            </TabsContent>

            <TabsContent value="tabs" className="mt-4 space-y-3">
              {COLOR_GROUPS.tabs.map((item) => (
                <ColorInput
                  key={item.key}
                  label={item.label}
                  value={colors[item.key as keyof ThemeColors]}
                  onChange={(value) => handleColorChange(item.key as keyof ThemeColors, value)}
                />
              ))}
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
                  {group.items.map((item) => (
                    <TerminalColorInput
                      key={item.key}
                      label={item.label}
                      value={terminal[item.key]}
                      onChange={(value) => handleTerminalChange(item.key, value)}
                    />
                  ))}
                </div>
              ))}
            </TabsContent>
          </Tabs>
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
}

const ColorInput: React.FC<ColorInputProps> = ({ label, value, onChange }) => {
  return (
    <div className="flex items-center gap-3">
      <div
        className="border-border size-8 shrink-0 rounded border"
        style={{ backgroundColor: normalizeColorPreview(value) }}
      />
      <div className="flex-1">
        <Label className="text-muted-foreground text-xs">{label}</Label>
        <Input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="0 0% 0%"
          className="h-8 font-mono text-sm"
        />
      </div>
    </div>
  )
}

const TerminalColorInput: React.FC<ColorInputProps> = ({ label, value, onChange }) => {
  return (
    <div className="flex items-center gap-3">
      <div
        className="border-border size-8 shrink-0 rounded border"
        style={{ backgroundColor: normalizeColorPreview(value) }}
      />
      <div className="flex-1">
        <Label className="text-muted-foreground text-xs">{label}</Label>
        <Input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="#000000 / rgba(...) / hsl(...)"
          className="h-8 font-mono text-sm"
        />
      </div>
    </div>
  )
}
