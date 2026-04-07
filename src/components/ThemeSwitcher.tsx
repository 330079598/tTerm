import React, {useState} from "react"
import {useTranslation} from "react-i18next"
import {Check, Copy, Edit, Palette, Plus, Trash2} from "lucide-react"
import {useTheme} from "@/contexts/ThemeContext"
import type {PresetThemeId} from "@/types/theme"
import {Button} from "@/components/ui/button"
import {Card, CardContent} from "@/components/ui/card"
import {Dialog, DialogContent, DialogHeader, DialogTitle} from "@/components/ui/dialog"
import {ThemeEditor} from "@/components/ThemeEditor"
import {cn, hslToCssColor} from "@/lib/utils"

interface ThemeSwitcherProps {
  onClose: () => void
}

const PRESET_THEME_COLORS: Record<PresetThemeId, string> = {
  default: "hsl(220 13% 12%)",
  light: "hsl(0 0% 98%)",
  ocean: "hsl(200 30% 10%)",
  forest: "hsl(140 25% 12%)",
  sunset: "hsl(20 30% 12%)",
  ubuntu: "hsl(300 100% 6%)",
}

export const ThemeSwitcher: React.FC<ThemeSwitcherProps> = ({ onClose }) => {
  const { t } = useTranslation()
  const { currentTheme, presetThemes, customThemes, setTheme, deleteCustomTheme, duplicateTheme } =
    useTheme()

  const [editingThemeId, setEditingThemeId] = useState<string | null>(null)
  const [creatingFromTheme, setCreatingFromTheme] = useState<string | null>(null)

  const handleThemeChange = async (themeId: string) => {
    try {
      await setTheme(themeId)
    } catch (error) {
      console.error("Failed to save theme:", error)
    }
  }

  const handleDeleteTheme = async (themeId: string) => {
    if (confirm(t("themeEditor.confirmDelete"))) {
      try {
        await deleteCustomTheme(themeId)
      } catch (error) {
        console.error("Failed to delete theme:", error)
      }
    }
  }

  const handleDuplicateTheme = async (themeId: string) => {
    const sourceName =
      presetThemes.find((t) => t.id === themeId)?.name ||
      customThemes.find((t) => t.id === themeId)?.name ||
      "Theme"
    const newName = prompt(t("themeEditor.duplicateName"), `${sourceName} Copy`)

    if (newName && newName.trim()) {
      try {
        await duplicateTheme(themeId, newName.trim())
      } catch (error) {
        console.error("Failed to duplicate theme:", error)
      }
    }
  }

  const getThemeColor = (themeId: string): string => {
    const customTheme = customThemes.find((t) => t.id === themeId)
    if (customTheme) {
      return hslToCssColor(customTheme.colors.background)
    }
    return PRESET_THEME_COLORS[themeId as PresetThemeId] || "hsl(0 0% 50%)"
  }

  return (
    <>
      <Dialog open onOpenChange={(open) => !open && onClose()}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Palette size={16} />
              {t("theme.title")}
            </DialogTitle>
          </DialogHeader>

          <div className="space-y-4 py-2">
            {/* Preset themes */}
            <div>
              <h3 className="mb-2 text-sm font-medium">{t("themeEditor.presetThemes")}</h3>
              <div className="grid grid-cols-1 gap-2">
                {presetThemes.map((theme) => (
                  <Card key={theme.id} className="overflow-hidden border-transparent shadow-none">
                    <CardContent className="p-0">
                      <div className="flex items-center">
                        <Button
                          type="button"
                          variant="ghost"
                          onClick={() => handleThemeChange(theme.id)}
                          className={cn(
                            "h-auto flex-1 justify-start gap-3 rounded-lg border px-3 py-2.5 text-left",
                            currentTheme === theme.id
                              ? "border-primary bg-accent"
                              : "border-transparent"
                          )}
                        >
                          <span
                            className="border-border size-5 shrink-0 rounded-full border"
                            style={{ background: getThemeColor(theme.id) }}
                          />
                          <div className="flex flex-col items-start">
                            <span className="text-sm leading-none font-medium">
                              {t(`theme.${theme.id}`)}
                            </span>
                            <span className="text-muted-foreground mt-1 text-xs">
                              {t(`theme.${theme.id}Desc`)}
                            </span>
                          </div>
                          {currentTheme === theme.id && (
                            <Check size={16} className="text-primary ml-auto" />
                          )}
                        </Button>
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          onClick={() => setCreatingFromTheme(theme.id)}
                          className="h-auto px-2 py-2"
                          title={t("themeEditor.customize")}
                        >
                          <Plus size={16} />
                        </Button>
                      </div>
                    </CardContent>
                  </Card>
                ))}
              </div>
            </div>

            {/* Custom themes */}
            {customThemes.length > 0 && (
              <div>
                <h3 className="mb-2 text-sm font-medium">{t("themeEditor.customThemes")}</h3>
                <div className="grid grid-cols-1 gap-2">
                  {customThemes.map((theme) => (
                    <Card key={theme.id} className="overflow-hidden border-transparent shadow-none">
                      <CardContent className="p-0">
                        <div className="flex items-center">
                          <Button
                            type="button"
                            variant="ghost"
                            onClick={() => handleThemeChange(theme.id)}
                            className={cn(
                              "h-auto flex-1 justify-start gap-3 rounded-lg border px-3 py-2.5 text-left",
                              currentTheme === theme.id
                                ? "border-primary bg-accent"
                                : "border-transparent"
                            )}
                          >
                            <span
                              className="border-border size-5 shrink-0 rounded-full border"
                              style={{ background: getThemeColor(theme.id) }}
                            />
                            <div className="flex flex-col items-start">
                              <span className="text-sm leading-none font-medium">{theme.name}</span>
                              {theme.description && (
                                <span className="text-muted-foreground mt-1 text-xs">
                                  {theme.description}
                                </span>
                              )}
                            </div>
                            {currentTheme === theme.id && (
                              <Check size={16} className="text-primary ml-auto" />
                            )}
                          </Button>
                          <div className="flex">
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              onClick={() => setEditingThemeId(theme.id)}
                              className="h-auto px-2 py-2"
                              title={t("themeEditor.edit")}
                            >
                              <Edit size={14} />
                            </Button>
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              onClick={() => handleDuplicateTheme(theme.id)}
                              className="h-auto px-2 py-2"
                              title={t("themeEditor.duplicate")}
                            >
                              <Copy size={14} />
                            </Button>
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              onClick={() => handleDeleteTheme(theme.id)}
                              className="text-destructive hover:text-destructive h-auto px-2 py-2"
                              title={t("themeEditor.delete")}
                            >
                              <Trash2 size={14} />
                            </Button>
                          </div>
                        </div>
                      </CardContent>
                    </Card>
                  ))}
                </div>
              </div>
            )}

            {/* Create new theme button */}
            <Button
              variant="outline"
              className="w-full"
              onClick={() => setCreatingFromTheme("default")}
            >
              <Plus size={16} className="mr-2" />
              {t("themeEditor.createNew")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* Theme editor */}
      {editingThemeId && (
        <ThemeEditor themeId={editingThemeId} onClose={() => setEditingThemeId(null)} />
      )}

      {/* Create based on preset theme */}
      {creatingFromTheme && (
        <ThemeEditor baseThemeId={creatingFromTheme} onClose={() => setCreatingFromTheme(null)} />
      )}
    </>
  )
}
