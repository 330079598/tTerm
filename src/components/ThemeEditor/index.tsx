import React, { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"
import { Palette, Save, Sparkles, X } from "lucide-react"

import { TerminalPalettePreview } from "@/components/TerminalPalettePreview"
import { ThemePreviewSwatches } from "@/components/ThemePreviewSwatches"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useTheme } from "@/contexts/ThemeContext"
import { generateTerminalPaletteFromColors } from "@/lib/terminalPalette"
import type { PresetThemeId, TerminalPalette, ThemeColors } from "@/types/theme"
import { PRESET_THEME_IDS } from "@/types/theme"

import { ColorInput, TerminalColorInput } from "@/components/ThemeEditor/ColorInput"
import { ThemeLivePreview } from "@/components/ThemeEditor/ThemeLivePreview"
import {
  createTerminalSuggestions,
  createThemeSuggestions,
} from "@/components/ThemeEditor/colorUtils"
import {
  COLOR_GROUPS,
  TERMINAL_GROUPS,
  terminalColorKey,
} from "@/components/ThemeEditor/themeEditorConstants"
import type {
  TerminalColorItem,
  ThemeColorItem,
  ThemeEditorProps,
} from "@/components/ThemeEditor/types"

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
        const baseName = isPresetThemeId(baseThemeId) ? t(`theme.${baseThemeId}`) : baseThemeId
        const themeData = themeUtils.createCustomThemeFromPreset(
          baseThemeId as PresetThemeId,
          t("themeEditor.defaultCustomName", { name: baseName }),
          t("themeEditor.defaultCustomDescription", { name: baseName })
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
      label={t(item.labelKey)}
      value={colors[item.key]}
      onChange={(value) => handleColorChange(item.key, value)}
      suggestions={themeSuggestions}
      token={item.token}
      description={t(item.descriptionKey)}
      isOpen={activeColorKey === item.key}
      onOpenChange={(open) => setActiveColorKey(open ? item.key : null)}
    />
  )

  const renderTerminalColorInput = (item: TerminalColorItem) => (
    <TerminalColorInput
      key={item.key}
      label={t(item.labelKey)}
      value={terminal[item.key]}
      onChange={(value) => handleTerminalChange(item.key, value)}
      suggestions={terminalSuggestions}
      isOpen={activeColorKey === terminalColorKey(item.key)}
      onOpenChange={(open) => setActiveColorKey(open ? terminalColorKey(item.key) : null)}
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
                <ThemeEditorTab value="basic" label={t("themeEditor.basic")} />
                <ThemeEditorTab value="components" label={t("themeEditor.components")} />
                <ThemeEditorTab value="status" label={t("themeEditor.status")} />
                <ThemeEditorTab value="tabs" label={t("themeEditor.tabs")} />
                <ThemeEditorTab value="terminal" label={t("themeEditor.terminal")} />
              </TabsList>

              <TabsContent value="basic" className="mt-4 space-y-3">
                {COLOR_GROUPS.basic.map(renderThemeColorInput)}
                {COLOR_GROUPS.input.map(renderThemeColorInput)}
              </TabsContent>

              <TabsContent value="components" className="mt-4 space-y-3">
                <ColorSection title={t("themeEditor.sections.card")}>
                  {COLOR_GROUPS.card.map(renderThemeColorInput)}
                </ColorSection>
                <ColorSection title={t("themeEditor.sections.primary")}>
                  {COLOR_GROUPS.primary.map(renderThemeColorInput)}
                </ColorSection>
                <ColorSection title={t("themeEditor.sections.secondary")}>
                  {COLOR_GROUPS.secondary.map(renderThemeColorInput)}
                </ColorSection>
                <ColorSection title={t("themeEditor.sections.accentMuted")}>
                  {[...COLOR_GROUPS.accent, ...COLOR_GROUPS.muted].map(renderThemeColorInput)}
                </ColorSection>
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
                      <h4 className="text-sm font-medium">{t("themeEditor.terminalPreview")}</h4>
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
                  <ColorSection key={group.titleKey} title={t(group.titleKey)}>
                    {group.items.map(renderTerminalColorInput)}
                  </ColorSection>
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

interface ThemeEditorTabProps {
  value: string
  label: string
}

const ThemeEditorTab: React.FC<ThemeEditorTabProps> = ({ value, label }) => {
  return (
    <TabsTrigger value={value} className="min-w-0 px-2 text-xs whitespace-normal sm:text-sm">
      {label}
    </TabsTrigger>
  )
}

interface ColorSectionProps {
  title: string
  children: React.ReactNode
}

const ColorSection: React.FC<ColorSectionProps> = ({ title, children }) => {
  return (
    <div className="space-y-2">
      <h4 className="text-sm font-medium">{title}</h4>
      {children}
    </div>
  )
}
