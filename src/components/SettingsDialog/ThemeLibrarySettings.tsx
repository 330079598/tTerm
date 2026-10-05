import React from "react"
import { useTranslation } from "react-i18next"
import { Copy, Moon, Star, Sun } from "lucide-react"

import { ThemeCard } from "@/components/ThemeCard"
import { SettingsRow } from "@/components/SettingsDialog/SettingsLayout"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useCatalogFor, useTheme } from "@/contexts/ThemeContext"
import { toast } from "@/hooks/use-toast"
import { catalogThemeName, isCatalogTheme, isCatalogThemeId } from "@/lib/themeCatalog"
import type { Theme, ThemeSlot } from "@/types/theme"
import { PRESET_THEME_IDS, type PresetThemeId } from "@/types/theme"

/** Runs a settings save, telling the user when it fails. */
function useSave() {
  const { t } = useTranslation()
  return (save: Promise<unknown>) => {
    save.catch((error: unknown) => {
      console.error("Failed to save theme settings:", error)
      toast({
        title: t("errors.configSaveFailed", { defaultValue: "Failed to save settings" }),
        variant: "destructive",
      })
    })
  }
}

/** A theme's name as the settings show it: built-in themes in the UI's language. */
function useThemeName() {
  const { t } = useTranslation()
  const { presetThemeOverrides } = useTheme()
  return (theme: Theme) =>
    PRESET_THEME_IDS.includes(theme.id as PresetThemeId) &&
    !presetThemeOverrides.some((override) => override.id === theme.id)
      ? t(`theme.${theme.id}`)
      : theme.name
}

interface ThemeSlotSelectProps {
  slot: ThemeSlot
}

function ThemeSlotSelect({ slot }: ThemeSlotSelectProps) {
  const { t } = useTranslation()
  const {
    customThemes,
    darkTheme,
    favoriteThemes,
    getTheme,
    isDarkTheme,
    lightTheme,
    presetThemes,
    setSystemTheme,
  } = useTheme()
  const themeName = useThemeName()
  const save = useSave()
  const value = slot === "dark" ? darkTheme : lightTheme
  const label =
    slot === "dark"
      ? t("theme.darkAppearance", { defaultValue: "Dark mode" })
      : t("theme.lightAppearance", { defaultValue: "Light mode" })

  // Themes of this appearance, plus whatever the setting names now.
  const candidates = [
    ...presetThemes,
    ...customThemes,
    ...favoriteThemes.map((themeId) => getTheme(themeId)).filter((theme) => !!theme),
  ].filter((theme) => isDarkTheme(theme.id) === (slot === "dark"))
  const selected = getTheme(value)
  if (selected && !candidates.some((theme) => theme.id === value)) candidates.unshift(selected)
  const options = candidates.map((theme) => ({ id: theme.id, name: themeName(theme) }))

  const id = `theme-slot-${slot}`
  return (
    <div className="flex min-w-0 items-center gap-3">
      <Label htmlFor={id} className="flex w-28 shrink-0 items-center gap-2 text-sm font-normal">
        {slot === "dark" ? <Moon size={14} /> : <Sun size={14} />}
        {label}
      </Label>
      <div className="min-w-0 flex-1">
        <Select
          id={id}
          value={value}
          onChange={(event) => save(setSystemTheme(slot, event.target.value))}
        >
          {!options.some((option) => option.id === value) && (
            <option value={value}>
              {isCatalogThemeId(value) ? catalogThemeName(value) : value}
            </option>
          )}
          {options.map((option) => (
            <option key={option.id} value={option.id}>
              {option.name}
            </option>
          ))}
        </Select>
      </div>
    </div>
  )
}

/** Switches between a light and a dark theme with the system's appearance. */
export function FollowSystemThemeSetting() {
  const { t } = useTranslation()
  const { darkTheme, followSystem, lightTheme, setFollowSystem } = useTheme()
  const save = useSave()
  useCatalogFor(followSystem ? [lightTheme, darkTheme] : [])

  return (
    <SettingsRow
      className="mb-4"
      title={t("theme.followSystem", { defaultValue: "Follow system appearance" })}
      description={t("theme.followSystemDesc", {
        defaultValue:
          "Switch between a light and a dark theme with the system. A theme you choose becomes the one for its own appearance.",
      })}
      action={
        <Switch
          aria-label={t("theme.followSystem", { defaultValue: "Follow system appearance" })}
          checked={followSystem}
          onCheckedChange={(enabled) => save(setFollowSystem(enabled))}
        />
      }
    >
      {followSystem && (
        <div className="grid gap-2">
          <ThemeSlotSelect slot="light" />
          <ThemeSlotSelect slot="dark" />
        </div>
      )}
    </SettingsRow>
  )
}

interface LibraryThemeCardsProps {
  currentTheme: string
  onCopy: (themeId: string) => void
  onSelect: (themeId: string) => void
}

/** The starred library themes, and the current one when it is not starred. */
export function LibraryThemeCards({ currentTheme, onCopy, onSelect }: LibraryThemeCardsProps) {
  const { t } = useTranslation()
  const { favoriteThemes, getTheme, toggleFavoriteTheme } = useTheme()
  const save = useSave()
  useCatalogFor(favoriteThemes)

  const themes = [
    ...(favoriteThemes.includes(currentTheme) ? [] : [currentTheme]),
    ...favoriteThemes,
  ]
    .map(getTheme)
    .filter(isCatalogTheme)

  if (themes.length === 0) return null

  const iconButton = (label: string, onClick: () => void, icon: React.ReactNode) => (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={onClick}
          aria-label={label}
          className="h-auto px-2 py-2"
        >
          {icon}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )

  return (
    <div className="mb-4">
      <h4 className="text-muted-foreground mb-2 text-xs font-medium">
        {t("themeGallery.libraryThemes", { defaultValue: "Theme library" })}
      </h4>
      <div className="grid gap-1.5">
        {themes.map((theme) => {
          const favorite = favoriteThemes.includes(theme.id)
          return (
            <ThemeCard
              key={theme.id}
              currentTheme={currentTheme}
              description={
                theme.isDark
                  ? t("themeGallery.dark", { defaultValue: "Dark" })
                  : t("themeGallery.light", { defaultValue: "Light" })
              }
              name={theme.name}
              onSelect={() => onSelect(theme.id)}
              compactPreview
              theme={theme}
              actionSlot={
                <div className="flex">
                  {iconButton(
                    favorite
                      ? t("themeGallery.unfavorite", { defaultValue: "Remove from favorites" })
                      : t("themeGallery.favorite", { defaultValue: "Add to favorites" }),
                    () => save(toggleFavoriteTheme(theme.id)),
                    <Star
                      size={14}
                      className={favorite ? "fill-current text-yellow-500" : undefined}
                    />
                  )}
                  {iconButton(
                    t("themeGallery.copy", { defaultValue: "Copy and edit" }),
                    () => onCopy(theme.id),
                    <Copy size={14} />
                  )}
                </div>
              }
            />
          )
        })}
      </div>
    </div>
  )
}
