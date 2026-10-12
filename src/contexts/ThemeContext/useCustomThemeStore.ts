import { invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import {
  isPresetThemeId,
  mergePresetThemeWithOverride,
  normalizeCustomThemes,
  readLegacyThemes,
  removeLegacyThemes,
} from "@/lib/customThemes"
import { onSyncApplied } from "@/lib/sync"
import { PRESET_THEMES } from "@/lib/themeDefinitions"
import type { CustomTheme } from "@/types/theme"

/**
 * The saved custom themes and preset overrides: loaded at start and after a
 * sync brings in themes, and saved as one list.
 */
export function useCustomThemeStore() {
  const [customThemes, setCustomThemes] = useState<CustomTheme[]>([])
  // The latest themes for calls made right after a save, from callbacks that
  // still hold the themes before it.
  const customThemesRef = useRef<CustomTheme[]>([])
  const [themesLoaded, setThemesLoaded] = useState(false)

  const presetThemeOverrides = useMemo(
    () => customThemes.filter((theme) => isPresetThemeId(theme.id)),
    [customThemes]
  )

  const standaloneCustomThemes = useMemo(
    () => customThemes.filter((theme) => !isPresetThemeId(theme.id)),
    [customThemes]
  )

  const presetThemes = useMemo(
    () =>
      PRESET_THEMES.map((theme) =>
        mergePresetThemeWithOverride(
          theme,
          presetThemeOverrides.find((overrideTheme) => overrideTheme.id === theme.id)
        )
      ),
    [presetThemeOverrides]
  )

  const loadCustomThemes = useCallback(async () => {
    try {
      // The first load hands the themes kept in localStorage to the database.
      const themes = await invoke<unknown[]>("load_custom_themes", { legacy: readLegacyThemes() })
      customThemesRef.current = normalizeCustomThemes(themes)
      setCustomThemes(customThemesRef.current)
      removeLegacyThemes()
    } catch (error) {
      console.error("Failed to load custom themes:", error)
      if (customThemesRef.current.length === 0) {
        customThemesRef.current = readLegacyThemes() ?? []
      }
      setCustomThemes(customThemesRef.current)
    } finally {
      setThemesLoaded(true)
    }
  }, [])

  useEffect(() => {
    void loadCustomThemes()
  }, [loadCustomThemes])

  useEffect(() => onSyncApplied(["themes"], () => void loadCustomThemes()), [loadCustomThemes])

  const saveCustomThemes = useCallback(async (themes: CustomTheme[]) => {
    try {
      await invoke("save_custom_themes", { themes })
      customThemesRef.current = themes
      setCustomThemes(themes)
    } catch (error) {
      console.error("Failed to save custom themes:", error)
      throw error
    }
  }, [])

  return {
    customThemes,
    customThemesRef,
    themesLoaded,
    presetThemeOverrides,
    standaloneCustomThemes,
    presetThemes,
    loadCustomThemes,
    saveCustomThemes,
  }
}
