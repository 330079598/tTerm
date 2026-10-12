import type { TFunction } from "i18next"
import React, { useCallback } from "react"

import type { useConfig } from "@/contexts/ConfigContext"
import type { AppConfig } from "@/lib/appConfig"
import { getPresetTheme } from "@/lib/themeDefinitions"
import type { CatalogTheme, CustomTheme, PresetThemeId } from "@/types/theme"

interface CustomThemeActionsOptions {
  customThemes: CustomTheme[]
  customThemesRef: React.RefObject<CustomTheme[]>
  saveCustomThemes: (themes: CustomTheme[]) => Promise<void>
  applyAndCacheTheme: (themeId: string, themes: CustomTheme[]) => void
  config: AppConfig
  saveConfig: ReturnType<typeof useConfig>["saveConfig"]
  knownCatalogThemes: CatalogTheme[]
  t: TFunction
}

/** Creating, editing, deleting, resetting and copying custom themes. */
export function useCustomThemeActions({
  customThemes,
  customThemesRef,
  saveCustomThemes,
  applyAndCacheTheme,
  config,
  saveConfig,
  knownCatalogThemes,
  t,
}: CustomThemeActionsOptions) {
  const createCustomThemes = useCallback(
    async (themes: Array<Omit<CustomTheme, "id">>): Promise<CustomTheme[]> => {
      const newThemes: CustomTheme[] = themes.map((theme) => ({
        ...theme,
        id: `custom-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      }))

      await saveCustomThemes([...customThemesRef.current, ...newThemes])

      return newThemes
    },
    [saveCustomThemes, customThemesRef]
  )

  const createCustomTheme = useCallback(
    async (theme: Omit<CustomTheme, "id">): Promise<CustomTheme> => {
      const [newTheme] = await createCustomThemes([theme])
      return newTheme
    },
    [createCustomThemes]
  )

  const updateCustomTheme = useCallback(
    async (id: string, updates: Partial<CustomTheme>) => {
      const presetTheme = getPresetTheme(id)
      const existingTheme = customThemes.find((theme) => theme.id === id)

      let updatedThemes: CustomTheme[]

      if (existingTheme) {
        updatedThemes = customThemes.map((theme) =>
          theme.id === id ? { ...theme, ...updates, updatedAt: Date.now() } : theme
        )
      } else if (presetTheme) {
        const createdTheme: CustomTheme = {
          id,
          name: updates.name ?? presetTheme.name,
          description: updates.description ?? presetTheme.description,
          colors: updates.colors ? { ...updates.colors } : { ...presetTheme.colors },
          terminal: updates.terminal ? { ...updates.terminal } : { ...presetTheme.terminal },
          baseTheme: presetTheme.id,
          isCustom: true,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        }

        updatedThemes = [...customThemes, createdTheme]
      } else {
        return
      }

      await saveCustomThemes(updatedThemes)

      if (config.theme === id) {
        applyAndCacheTheme(id, updatedThemes)
      }
    },
    [applyAndCacheTheme, config.theme, customThemes, saveCustomThemes]
  )

  const deleteCustomTheme = useCallback(
    async (id: string) => {
      const updatedThemes = customThemes.filter((theme) => theme.id !== id)
      await saveCustomThemes(updatedThemes)

      // Every setting naming the theme falls back to a built-in one.
      const update: Partial<AppConfig> = {}
      if (config.theme === id) update.theme = "default"
      if (config.theme_light === id) update.theme_light = "light"
      if (config.theme_dark === id) update.theme_dark = "default"
      if (config.favorite_themes.includes(id)) {
        update.favorite_themes = config.favorite_themes.filter((themeId) => themeId !== id)
      }
      if (Object.keys(update).length > 0) await saveConfig(update)
    },
    [config, customThemes, saveConfig, saveCustomThemes]
  )

  const resetPresetTheme = useCallback(
    async (id: PresetThemeId) => {
      const updatedThemes = customThemes.filter((theme) => theme.id !== id)
      await saveCustomThemes(updatedThemes)

      if (config.theme === id) {
        applyAndCacheTheme(id, updatedThemes)
      }
    },
    [applyAndCacheTheme, config.theme, customThemes, saveCustomThemes]
  )

  const duplicateTheme = useCallback(
    async (themeId: string, newName: string): Promise<CustomTheme> => {
      const sourceTheme = customThemes.find((theme) => theme.id === themeId)
      const catalogTheme = knownCatalogThemes.find((theme) => theme.id === themeId)

      if (catalogTheme) {
        return createCustomTheme({
          name: newName,
          description: t("theme.basedOn", { themeId: catalogTheme.name }),
          colors: { ...catalogTheme.colors },
          terminal: { ...catalogTheme.terminal },
          baseTheme: catalogTheme.id,
          isCustom: true,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        })
      }

      if (sourceTheme) {
        return createCustomTheme({
          name: newName,
          description: sourceTheme.description,
          colors: { ...sourceTheme.colors },
          terminal: { ...sourceTheme.terminal },
          baseTheme: sourceTheme.baseTheme,
          isCustom: true,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        })
      }

      const themeUtils = await import("@/lib/themeUtils")
      const themeData = themeUtils.createCustomThemeFromPreset(
        themeId as PresetThemeId,
        newName,
        t("theme.basedOn", { themeId, defaultValue: "Based on {{themeId}}" })
      )
      return createCustomTheme(themeData)
    },
    [createCustomTheme, customThemes, knownCatalogThemes, t]
  )

  return {
    createCustomThemes,
    createCustomTheme,
    updateCustomTheme,
    deleteCustomTheme,
    resetPresetTheme,
    duplicateTheme,
  }
}
