import React, { useEffect } from "react"
import { useTranslation } from "react-i18next"

import { SearchableSelect, type SearchableSelectGroup } from "@/components/ui/searchable-select"
import { Spinner } from "@/components/ui/spinner"
import { useTheme } from "@/contexts/ThemeContext"
import { catalogThemeName, isCatalogThemeId } from "@/lib/themeCatalog"
import { cn } from "@/lib/utils"
import { PRESET_THEME_IDS, type TerminalPalette, type Theme } from "@/types/theme"

interface TerminalThemePickerProps {
  id: string
  /** Theme id; empty follows the app theme. */
  value: string
  onChange: (themeId: string) => void
}

/** The theme's background with a sample of its text color. */
const Swatch: React.FC<{ palette?: TerminalPalette }> = ({ palette }) => (
  <span
    aria-hidden="true"
    className={cn(
      "flex h-4 w-6 shrink-0 items-center justify-center rounded-sm border font-mono text-[9px] leading-none",
      !palette && "border-dashed"
    )}
    style={palette ? { background: palette.background, color: palette.foreground } : undefined}
  >
    {palette ? "A" : ""}
  </span>
)

/**
 * Picks a theme for a connection's terminal from a searchable list, since the
 * theme library holds hundreds of themes.
 */
export const TerminalThemePicker: React.FC<TerminalThemePickerProps> = ({
  id,
  value,
  onChange,
}) => {
  const { t } = useTranslation()
  const {
    catalogThemes,
    customThemes,
    favoriteThemes,
    getTheme,
    loadCatalogThemes,
    presetThemeOverrides,
    presetThemes,
  } = useTheme()

  // The library is loaded on demand; the list offers all of it.
  useEffect(() => {
    if (catalogThemes) return
    loadCatalogThemes().catch((error: unknown) => {
      console.error("Failed to load the theme library:", error)
    })
  }, [catalogThemes, loadCatalogThemes])

  const themeLabel = (theme: Theme) => {
    const isPreset = (PRESET_THEME_IDS as readonly string[]).includes(theme.id)
    const hasOverride = presetThemeOverrides.some((override) => override.id === theme.id)
    return isPreset && !hasOverride ? t(`theme.${theme.id}`) : theme.name
  }

  const followAppLabel = t("connection.terminalThemeFollowApp")
  const groups: SearchableSelectGroup[] = [
    {
      label: null,
      options: [{ value: "", label: followAppLabel, icon: <Swatch /> }],
    },
    ...[
      { label: t("themeEditor.presetThemes"), themes: presetThemes },
      { label: t("themeEditor.customThemes"), themes: customThemes },
      {
        label: t("themeGallery.favorites"),
        themes: favoriteThemes
          .map((themeId) => getTheme(themeId))
          .filter((theme): theme is Theme => !!theme),
      },
      { label: t("themeGallery.libraryThemes"), themes: catalogThemes ?? [] },
    ].map((group) => ({
      label: group.label,
      options: group.themes.map((theme) => ({
        value: theme.id,
        label: themeLabel(theme),
        icon: <Swatch palette={theme.terminal} />,
      })),
    })),
  ]

  const selectedTheme = value ? getTheme(value) : undefined
  const selectedLabel = !value
    ? followAppLabel
    : selectedTheme
      ? themeLabel(selectedTheme)
      : isCatalogThemeId(value) && !catalogThemes
        ? catalogThemeName(value)
        : t("connection.terminalThemeMissing", { id: value })

  return (
    <SearchableSelect
      id={id}
      value={value}
      groups={groups}
      onChange={onChange}
      triggerLabel={selectedLabel}
      triggerIcon={<Swatch palette={selectedTheme?.terminal} />}
      searchLabel={t("themeGallery.search")}
      listLabel={t("connection.terminalTheme")}
      // The library's themes are still to come while it loads.
      emptyText={catalogThemes ? t("themeGallery.noMatch") : undefined}
      footer={
        !catalogThemes && (
          <div className="text-muted-foreground flex items-center gap-2 px-2 py-2 text-xs">
            <Spinner size={14} />
            {t("themeGallery.loading")}
          </div>
        )
      }
    />
  )
}
