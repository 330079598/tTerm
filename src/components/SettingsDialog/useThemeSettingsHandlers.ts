import type { TFunction } from "i18next"

import type { WindowBlurMaterial } from "@/lib/appConfig"
import type { useToast } from "@/hooks/use-toast"
import type { useTheme } from "@/contexts/ThemeContext"
import type { useConfirmDialog, usePromptDialog } from "@/components/ui/app-dialog"
import type { useSettingsSave } from "@/hooks/useSettingsSave"
import type { TabWidthMode } from "@/lib/tabSettings"
import { isCatalogTheme } from "@/lib/themeCatalog"
import type { CustomTheme, PresetThemeId } from "@/types/theme"

interface UseThemeSettingsHandlersOptions {
  setTheme: ReturnType<typeof useTheme>["setTheme"]
  systemPrefersDark: ReturnType<typeof useTheme>["systemPrefersDark"]
  toast: ReturnType<typeof useToast>["toast"]
  t: TFunction
  saveSettings: ReturnType<typeof useSettingsSave>["saveSettings"]
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  deleteCustomTheme: ReturnType<typeof useTheme>["deleteCustomTheme"]
  prompt: ReturnType<typeof usePromptDialog>["prompt"]
  duplicateTheme: ReturnType<typeof useTheme>["duplicateTheme"]
  getTheme: ReturnType<typeof useTheme>["getTheme"]
  presetThemes: ReturnType<typeof useTheme>["presetThemes"]
  customThemes: ReturnType<typeof useTheme>["customThemes"]
  setThemeGalleryOpen: (open: boolean) => void
  setEditingThemeId: (themeId: string | null) => void
  resetPresetTheme: ReturnType<typeof useTheme>["resetPresetTheme"]
}

/**
 * The appearance tab's actions: choosing, copying, deleting and resetting
 * themes, and saving the window and tab settings.
 */
export function useThemeSettingsHandlers({
  setTheme,
  systemPrefersDark,
  toast,
  t,
  saveSettings,
  confirm,
  deleteCustomTheme,
  prompt,
  duplicateTheme,
  getTheme,
  presetThemes,
  customThemes,
  setThemeGalleryOpen,
  setEditingThemeId,
  resetPresetTheme,
}: UseThemeSettingsHandlersOptions) {
  const handleThemeChange = async (themeId: string) => {
    try {
      const slot = await setTheme(themeId)
      // Following the system, a theme for the other appearance waits for it.
      if (slot && (slot === "dark") !== systemPrefersDark) {
        toast({
          title:
            slot === "dark"
              ? t("theme.savedForDark", { defaultValue: "Set as the dark mode theme" })
              : t("theme.savedForLight", { defaultValue: "Set as the light mode theme" }),
          description:
            slot === "dark"
              ? t("theme.savedForDarkDesc", {
                  defaultValue: "It shows when the system switches to dark mode.",
                })
              : t("theme.savedForLightDesc", {
                  defaultValue: "It shows when the system switches to light mode.",
                }),
        })
      }
    } catch (error) {
      console.error("Failed to save theme:", error)
      toast({
        title: t("common.error", { defaultValue: "Error" }),
        description: t("settings.themeChangeFailed", {
          defaultValue: "Failed to save theme.",
        }),
        variant: "destructive",
      })
    }
  }

  const handleTabWidthModeChange = async (mode: TabWidthMode) => {
    await saveSettings({ tab_width_mode: mode })
  }

  const handleTabStandardWidthChange = (width: number) =>
    saveSettings({ tab_standard_width: width })

  const handleUiScaleChange = (scale: number) => saveSettings({ ui_scale_percent: scale })

  const handleWindowBlurChange = (enabled: boolean) => saveSettings({ window_blur: enabled })

  const handleWindowBlurMaterialChange = (material: WindowBlurMaterial) =>
    saveSettings({ window_blur_material: material })

  const handleWindowBlurRadiusChange = (radius: number) =>
    saveSettings({ window_blur_radius: radius })

  const handleWindowOpacityChange = (percent: number) =>
    saveSettings({ window_opacity_percent: percent })

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
      toast({
        title: t("common.error", { defaultValue: "Error" }),
        description: t("themeEditor.deleteFailed", {
          defaultValue: "Failed to delete theme.",
        }),
        variant: "destructive",
      })
    }
  }

  /** Asks for a name and copies the theme; `null` when cancelled or failed. */
  const duplicateWithName = async (
    themeId: string,
    defaultName: string
  ): Promise<CustomTheme | null> => {
    const newName = await prompt({
      title: t("themeEditor.duplicate"),
      label: t("themeEditor.duplicateName"),
      defaultValue: defaultName,
      confirmText: t("themeEditor.duplicate"),
      cancelText: t("common.cancel"),
    })

    if (!newName || !newName.trim()) return null
    try {
      return await duplicateTheme(themeId, newName.trim())
    } catch (error) {
      console.error("Failed to duplicate theme:", error)
      toast({
        title: t("common.error", { defaultValue: "Error" }),
        description: t("themeEditor.duplicateFailed", {
          defaultValue: "Failed to duplicate theme.",
        }),
        variant: "destructive",
      })
      return null
    }
  }

  const handleDuplicateTheme = async (themeId: string) => {
    if (isCatalogTheme(getTheme(themeId))) {
      await handleCopyCatalogTheme(themeId)
      return
    }
    const sourceName =
      presetThemes.find((theme) => theme.id === themeId)?.name ||
      customThemes.find((theme) => theme.id === themeId)?.name ||
      "Theme"
    await duplicateWithName(themeId, `${sourceName} Copy`)
  }

  /** A library theme becomes a custom one, chosen and opened in the editor. */
  const handleCopyCatalogTheme = async (themeId: string) => {
    const copy = await duplicateWithName(themeId, getTheme(themeId)?.name ?? "Theme")
    if (!copy) return
    await handleThemeChange(copy.id)
    setThemeGalleryOpen(false)
    setEditingThemeId(copy.id)
  }

  const handleGalleryApply = async (themeId: string) => {
    await handleThemeChange(themeId)
    setThemeGalleryOpen(false)
  }

  const handleThemesImported = async (themes: CustomTheme[]) => {
    await handleThemeChange(themes[0].id)
    toast({
      title: t("themeImport.imported", {
        defaultValue: "Imported {{count}} themes",
        count: themes.length,
      }),
    })
  }

  const handleResetPresetTheme = async (themeId: PresetThemeId) => {
    try {
      await resetPresetTheme(themeId)
    } catch (error) {
      console.error("Failed to reset preset theme:", error)
      toast({
        title: t("common.error", { defaultValue: "Error" }),
        description: t("themeEditor.resetFailed", {
          defaultValue: "Failed to reset theme.",
        }),
        variant: "destructive",
      })
    }
  }

  return {
    handleThemeChange,
    handleTabWidthModeChange,
    handleTabStandardWidthChange,
    handleUiScaleChange,
    handleWindowBlurChange,
    handleWindowBlurMaterialChange,
    handleWindowBlurRadiusChange,
    handleWindowOpacityChange,
    handleDeleteTheme,
    handleDuplicateTheme,
    handleCopyCatalogTheme,
    handleGalleryApply,
    handleThemesImported,
    handleResetPresetTheme,
  }
}
