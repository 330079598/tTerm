import React, { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"
import { Palette, Save, X } from "lucide-react"
import { useTheme } from "@/contexts/ThemeContext"
import type { CustomTheme, PresetThemeId, ThemeColors } from "@/types/theme"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

interface ThemeEditorProps {
  themeId?: string // If provided, edit existing theme; otherwise create new theme
  baseThemeId?: string // Which theme to base on
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
}

export const ThemeEditor: React.FC<ThemeEditorProps> = ({
  themeId,
  baseThemeId,
  onClose,
  onSave,
}) => {
  const { t } = useTranslation()
  const { getTheme, createCustomTheme, updateCustomTheme } = useTheme()

  const [name, setName] = useState("")
  const [description, setDescription] = useState("")
  const [colors, setColors] = useState<ThemeColors | null>(null)
  const [isSaving, setIsSaving] = useState(false)

  useEffect(() => {
    const initializeTheme = async () => {
      if (themeId) {
        // Edit existing theme
        const theme = getTheme(themeId)
        if (theme && theme.isCustom) {
          setName(theme.name)
          setDescription(theme.description || "")
          setColors(theme.colors)
        }
      } else if (baseThemeId) {
        // Create based on preset theme
        const themeUtils = await import("@/lib/themeUtils")
        const themeData = themeUtils.createCustomThemeFromPreset(
          baseThemeId as PresetThemeId,
          `Custom ${baseThemeId}`,
          `Based on ${baseThemeId}`
        )
        setName(themeData.name)
        setDescription(themeData.description || "")
        setColors(themeData.colors)
      }
    }

    initializeTheme()
  }, [themeId, baseThemeId, getTheme])

  const handleColorChange = (key: keyof ThemeColors, value: string) => {
    if (colors) {
      setColors({ ...colors, [key]: value })
    }
  }

  const handleSave = async () => {
    if (!colors || !name.trim()) return

    setIsSaving(true)
    try {
      if (themeId) {
        // Update existing theme
        await updateCustomTheme(themeId, {
          name: name.trim(),
          description: description.trim(),
          colors,
        })
        const theme = getTheme(themeId) as CustomTheme
        onSave?.(theme)
      } else {
        // Create new theme
        const newTheme = await createCustomTheme({
          name: name.trim(),
          description: description.trim(),
          colors,
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

  if (!colors) {
    return null
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="flex max-h-[90vh] max-w-3xl flex-col overflow-hidden">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Palette size={16} />
            {themeId ? t("themeEditor.editTheme") : t("themeEditor.createTheme")}
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 space-y-4 overflow-y-auto py-2">
          {/* Basic information */}
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

          {/* Color editor */}
          <Tabs defaultValue="basic" className="w-full">
            <TabsList className="grid w-full grid-cols-4">
              <TabsTrigger value="basic">{t("themeEditor.basic")}</TabsTrigger>
              <TabsTrigger value="components">{t("themeEditor.components")}</TabsTrigger>
              <TabsTrigger value="status">{t("themeEditor.status")}</TabsTrigger>
              <TabsTrigger value="tabs">{t("themeEditor.tabs")}</TabsTrigger>
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
          </Tabs>
        </div>

        {/* Action buttons */}
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

// Color input component
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
        style={{ backgroundColor: `hsl(${value})` }}
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
