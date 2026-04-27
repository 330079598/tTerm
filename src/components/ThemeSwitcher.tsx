import React, { useState } from "react"
import { useTranslation } from "react-i18next"
import { Copy, Edit, Palette, Plus, RotateCcw, Trash2 } from "lucide-react"

import { ThemeCard } from "@/components/ThemeCard"
import { ThemeEditor } from "@/components/ThemeEditor"
import { Button } from "@/components/ui/button"
import { useConfirmDialog, usePromptDialog } from "@/components/ui/app-dialog"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { useTheme } from "@/contexts/ThemeContext"
import type { PresetThemeId } from "@/types/theme"

interface ThemeSwitcherProps {
  onClose: () => void
}

export const ThemeSwitcher: React.FC<ThemeSwitcherProps> = ({ onClose }) => {
  const { t } = useTranslation()
  const {
    currentTheme,
    presetThemes,
    customThemes,
    presetThemeOverrides,
    setTheme,
    deleteCustomTheme,
    resetPresetTheme,
    duplicateTheme,
  } = useTheme()

  const [editingThemeId, setEditingThemeId] = useState<string | null>(null)
  const [creatingFromTheme, setCreatingFromTheme] = useState<string | null>(null)
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const { prompt, PromptDialog } = usePromptDialog()

  const handleThemeChange = async (themeId: string) => {
    try {
      await setTheme(themeId)
    } catch (error) {
      console.error("Failed to save theme:", error)
    }
  }

  const handleDeleteTheme = async (themeId: string) => {
    const confirmed = await confirm({
      title: t("themeEditor.delete"),
      description: t("themeEditor.confirmDelete"),
      confirmText: t("themeEditor.delete"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })

    if (!confirmed) return

    try {
      await deleteCustomTheme(themeId)
    } catch (error) {
      console.error("Failed to delete theme:", error)
    }
  }

  const handleDuplicateTheme = async (themeId: string) => {
    const sourceName =
      presetThemes.find((theme) => theme.id === themeId)?.name ||
      customThemes.find((theme) => theme.id === themeId)?.name ||
      "Theme"
    const newName = await prompt({
      title: t("themeEditor.duplicate"),
      label: t("themeEditor.duplicateName"),
      defaultValue: `${sourceName} Copy`,
      confirmText: t("themeEditor.duplicate"),
      cancelText: t("common.cancel"),
    })

    if (newName && newName.trim()) {
      try {
        await duplicateTheme(themeId, newName.trim())
      } catch (error) {
        console.error("Failed to duplicate theme:", error)
      }
    }
  }

  const handleResetPresetTheme = async (themeId: PresetThemeId) => {
    try {
      await resetPresetTheme(themeId)
    } catch (error) {
      console.error("Failed to reset preset theme:", error)
    }
  }

  return (
    <>
      <Dialog open onOpenChange={(open) => !open && onClose()}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Palette size={16} />
              {t("theme.title")}
            </DialogTitle>
          </DialogHeader>

          <div className="space-y-4 py-2">
            <div>
              <h3 className="mb-2 text-sm font-medium">{t("themeEditor.presetThemes")}</h3>
              <div className="grid grid-cols-1 gap-2">
                {presetThemes.map((theme) => {
                  const hasOverride = presetThemeOverrides.some(
                    (override) => override.id === theme.id
                  )

                  return (
                    <ThemeCard
                      key={theme.id}
                      compactPreview
                      currentTheme={currentTheme}
                      description={
                        hasOverride
                          ? (theme.description ?? t("themeEditor.noDescription"))
                          : t(`theme.${theme.id}Desc`)
                      }
                      name={hasOverride ? theme.name : t(`theme.${theme.id}`)}
                      onSelect={() => handleThemeChange(theme.id)}
                      theme={theme}
                      actionSlot={
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
                          {hasOverride && (
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              onClick={() => handleResetPresetTheme(theme.id as PresetThemeId)}
                              className="h-auto px-2 py-2"
                              title={t("themeEditor.restorePreset")}
                            >
                              <RotateCcw size={14} />
                            </Button>
                          )}
                        </div>
                      }
                    />
                  )
                })}
              </div>
            </div>

            {customThemes.length > 0 && (
              <div>
                <h3 className="mb-2 text-sm font-medium">{t("themeEditor.customThemes")}</h3>
                <div className="grid grid-cols-1 gap-2">
                  {customThemes.map((theme) => (
                    <ThemeCard
                      key={theme.id}
                      compactPreview
                      currentTheme={currentTheme}
                      description={theme.description || t("themeEditor.noDescription")}
                      name={theme.name}
                      onSelect={() => handleThemeChange(theme.id)}
                      theme={theme}
                      actionSlot={
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
                      }
                    />
                  ))}
                </div>
              </div>
            )}

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

      {editingThemeId && (
        <ThemeEditor themeId={editingThemeId} onClose={() => setEditingThemeId(null)} />
      )}
      {creatingFromTheme && (
        <ThemeEditor baseThemeId={creatingFromTheme} onClose={() => setCreatingFromTheme(null)} />
      )}
      <ConfirmDialog />
      <PromptDialog />
    </>
  )
}
