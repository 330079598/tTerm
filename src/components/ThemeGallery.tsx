import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"
import { openUrl } from "@tauri-apps/plugin-opener"
import { Check, Copy, GalleryHorizontal, Loader2, RotateCcw, Search, Star } from "lucide-react"

import { TerminalPalettePreview } from "@/components/TerminalPalettePreview"
import { ThemePreviewSwatches } from "@/components/ThemePreviewSwatches"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { useTheme } from "@/contexts/ThemeContext"
import { useLatestRef } from "@/hooks/useLatestRef"
import { toast } from "@/hooks/use-toast"
import { isImeKeyEvent } from "@/lib/ime"
import { THEME_CATALOG_SOURCE } from "@/lib/themeCatalog"
import { cn } from "@/lib/utils"
import type { CatalogTheme } from "@/types/theme"

type Filter = "all" | "dark" | "light" | "favorites"

const FILTERS: Filter[] = ["all", "dark", "light", "favorites"]

interface ThemeGalleryProps {
  onApply: (themeId: string) => Promise<void>
  /** Copies a library theme into the custom themes, to be edited there. */
  onCopy: (themeId: string) => Promise<void>
  onClose: () => void
}

/**
 * The theme library, browsed like `ghostty +list-themes`: type to filter,
 * move with the arrow keys and the whole app shows the highlighted theme
 * until one is applied or the dialog is closed.
 */
export const ThemeGallery: React.FC<ThemeGalleryProps> = ({ onApply, onCopy, onClose }) => {
  const { t } = useTranslation()
  const {
    catalogThemes,
    currentTheme,
    favoriteThemes,
    loadCatalogThemes,
    previewTheme,
    toggleFavoriteTheme,
  } = useTheme()
  const [query, setQuery] = useState("")
  const [filter, setFilter] = useState<Filter>("all")
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [loadFailed, setLoadFailed] = useState(false)
  const [busy, setBusy] = useState(false)
  const listRef = useRef<HTMLDivElement>(null)

  const load = useCallback(
    () =>
      loadCatalogThemes().catch((error: unknown) => {
        console.error("Failed to load the theme library:", error)
        setLoadFailed(true)
      }),
    [loadCatalogThemes]
  )

  useEffect(() => {
    if (!catalogThemes) void load()
  }, [catalogThemes, load])

  const retry = () => {
    setLoadFailed(false)
    void load()
  }

  // Leaving without applying goes back to the current theme.
  useEffect(() => () => previewTheme(null), [previewTheme])

  const visibleThemes = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean)
    const fits = (theme: CatalogTheme) =>
      filter === "all" ||
      (filter === "favorites"
        ? favoriteThemes.includes(theme.id)
        : theme.isDark === (filter === "dark"))
    return (catalogThemes ?? []).filter(
      (theme) => fits(theme) && words.every((word) => theme.name.toLowerCase().includes(word))
    )
  }, [catalogThemes, favoriteThemes, filter, query])

  const selectedTheme = catalogThemes?.find((theme) => theme.id === selectedId)

  const select = useCallback(
    (theme: CatalogTheme) => {
      setSelectedId(theme.id)
      previewTheme(theme.id)
    },
    [previewTheme]
  )

  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>('[aria-selected="true"]')
      ?.scrollIntoView({ block: "nearest" })
  }, [selectedId])

  const run = async (action: (themeId: string) => Promise<void>) => {
    if (!selectedTheme || busy) return
    setBusy(true)
    try {
      await action(selectedTheme.id)
    } finally {
      setBusy(false)
    }
  }

  // Stable for the memoized rows; a double click has selected the row first.
  const applyRef = useRef(() => {})
  useLatestRef(applyRef, () => void run(onApply))
  const applySelected = useCallback(() => applyRef.current(), [])

  const toggleFavorite = useCallback(
    (themeId: string) => {
      toggleFavoriteTheme(themeId).catch((error: unknown) => {
        console.error("Failed to save favorite themes:", error)
        toast({
          title: t("errors.configSaveFailed", { defaultValue: "Failed to save settings" }),
          variant: "destructive",
        })
      })
    },
    [t, toggleFavoriteTheme]
  )

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (visibleThemes.length === 0 || isImeKeyEvent(event.nativeEvent)) return
    const index = visibleThemes.findIndex((theme) => theme.id === selectedId)
    const last = visibleThemes.length - 1
    const target = {
      ArrowDown: index === -1 ? 0 : Math.min(last, index + 1),
      ArrowUp: index === -1 ? last : Math.max(0, index - 1),
      PageDown: Math.min(last, Math.max(index, 0) + 10),
      PageUp: Math.max(0, index - 10),
    }[event.key]
    if (target !== undefined) {
      event.preventDefault()
      select(visibleThemes[target])
    } else if (event.key === "Enter" && selectedTheme) {
      event.preventDefault()
      void run(onApply)
    }
  }

  const filterLabel = (value: Filter) =>
    ({
      all: t("themeGallery.all", { defaultValue: "All" }),
      dark: t("themeGallery.dark", { defaultValue: "Dark" }),
      light: t("themeGallery.light", { defaultValue: "Light" }),
      favorites: t("themeGallery.favorites", { defaultValue: "Favorites" }),
    })[value]

  const selectedIsFavorite = !!selectedTheme && favoriteThemes.includes(selectedTheme.id)

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="flex h-[min(44rem,calc(100vh-2rem))] w-[min(64rem,calc(100vw-2rem))] flex-col gap-3 overflow-hidden sm:max-w-5xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <GalleryHorizontal size={16} />
            {t("themeGallery.title", { defaultValue: "Theme Library" })}
          </DialogTitle>
        </DialogHeader>

        <div className="flex flex-wrap items-center gap-2" onKeyDown={handleKeyDown}>
          <div className="relative min-w-48 flex-1">
            <Search
              size={14}
              className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2"
            />
            <Input
              autoFocus
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t("themeGallery.search", { defaultValue: "Search themes" })}
              aria-label={t("themeGallery.search", { defaultValue: "Search themes" })}
              className="pl-8"
            />
          </div>
          <div className="bg-muted/50 flex rounded-md p-0.5" role="group">
            {FILTERS.map((value) => (
              <Button
                key={value}
                type="button"
                size="sm"
                variant={filter === value ? "secondary" : "ghost"}
                aria-pressed={filter === value}
                className="h-7 px-3"
                onClick={() => setFilter(value)}
              >
                {value === "favorites" && <Star size={12} />}
                {filterLabel(value)}
                {value === "favorites" && favoriteThemes.length > 0 && ` ${favoriteThemes.length}`}
              </Button>
            ))}
          </div>
        </div>

        <div className="grid min-h-0 flex-1 gap-4 md:grid-cols-[18rem_minmax(0,1fr)]">
          <div className="flex min-h-0 flex-col rounded-lg border">
            <div className="text-muted-foreground border-b px-3 py-1.5 text-xs">
              {catalogThemes
                ? t("themeGallery.count", {
                    defaultValue: "{{count}} themes",
                    count: visibleThemes.length,
                  })
                : t("themeGallery.loading", { defaultValue: "Loading themes…" })}
            </div>
            <div
              ref={listRef}
              role="listbox"
              aria-label={t("themeGallery.title", { defaultValue: "Theme Library" })}
              tabIndex={-1}
              onKeyDown={handleKeyDown}
              className="min-h-0 flex-1 overflow-y-auto p-1"
            >
              {loadFailed ? (
                <div className="flex flex-col items-center gap-2 p-6 text-center text-sm">
                  <span className="text-destructive">
                    {t("themeGallery.loadFailed", {
                      defaultValue: "The theme library did not load.",
                    })}
                  </span>
                  <Button type="button" size="sm" variant="outline" onClick={retry}>
                    <RotateCcw size={14} />
                    {t("common.retry", { defaultValue: "Retry" })}
                  </Button>
                </div>
              ) : !catalogThemes ? (
                <div className="flex justify-center p-6">
                  <Loader2 className="text-muted-foreground size-5 animate-spin" />
                </div>
              ) : visibleThemes.length === 0 ? (
                <p className="text-muted-foreground p-6 text-center text-sm">
                  {filter === "favorites" && favoriteThemes.length === 0
                    ? t("themeGallery.noFavorites", {
                        defaultValue: "No favorites yet. Star a theme to keep it here.",
                      })
                    : t("themeGallery.noMatch", { defaultValue: "No theme matches." })}
                </p>
              ) : (
                visibleThemes.map((theme) => (
                  <CatalogThemeRow
                    key={theme.id}
                    theme={theme}
                    current={theme.id === currentTheme}
                    favorite={favoriteThemes.includes(theme.id)}
                    selected={theme.id === selectedId}
                    onSelect={select}
                    onApply={applySelected}
                    onToggleFavorite={toggleFavorite}
                  />
                ))
              )}
            </div>
          </div>

          <div className="flex min-h-0 min-w-0 flex-col gap-3 overflow-y-auto">
            {selectedTheme ? (
              <>
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="flex min-w-0 items-center gap-2">
                    <h3 className="truncate text-base font-semibold">{selectedTheme.name}</h3>
                    <Badge variant="outline">
                      {filterLabel(selectedTheme.isDark ? "dark" : "light")}
                    </Badge>
                    {selectedTheme.id === currentTheme && (
                      <Badge>{t("themeGallery.current", { defaultValue: "Current" })}</Badge>
                    )}
                  </div>
                  <div className="flex items-center gap-2">
                    <ThemePreviewSwatches compact palette={selectedTheme.terminal} />
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      aria-pressed={selectedIsFavorite}
                      onClick={() => toggleFavorite(selectedTheme.id)}
                    >
                      <Star
                        size={14}
                        className={selectedIsFavorite ? "fill-current text-yellow-500" : undefined}
                      />
                      {selectedIsFavorite
                        ? t("themeGallery.unfavorite", { defaultValue: "Remove from favorites" })
                        : t("themeGallery.favorite", { defaultValue: "Add to favorites" })}
                    </Button>
                  </div>
                </div>
                <div className="overflow-hidden rounded-lg border">
                  <TerminalPalettePreview focus={false} palette={selectedTheme.terminal} />
                </div>
                <p className="text-muted-foreground text-xs">
                  {t("themeGallery.previewHint", {
                    defaultValue:
                      "The whole app shows this theme while you browse. Close the library to go back.",
                  })}
                </p>
              </>
            ) : (
              <div className="text-muted-foreground flex flex-1 items-center justify-center rounded-lg border border-dashed p-6 text-center text-sm">
                {t("themeGallery.pickHint", {
                  defaultValue:
                    "Pick a theme, or use ↑ ↓ to browse; the app previews it right away.",
                })}
              </div>
            )}
          </div>
        </div>

        <div className="flex flex-wrap items-center justify-between gap-2 border-t pt-3">
          <button
            type="button"
            className="text-muted-foreground hover:text-foreground text-xs underline-offset-2 hover:underline"
            onClick={() => void openUrl(THEME_CATALOG_SOURCE)}
          >
            {t("themeGallery.source", {
              defaultValue: "Themes from iTerm2-Color-Schemes (MIT)",
            })}
          </button>
          <div className="flex gap-2">
            <Button type="button" variant="outline" onClick={onClose}>
              {t("common.cancel")}
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={!selectedTheme || busy}
              onClick={() => void run(onCopy)}
            >
              <Copy size={14} />
              {t("themeGallery.copy", { defaultValue: "Copy and edit" })}
            </Button>
            <Button
              type="button"
              disabled={!selectedTheme || busy}
              onClick={() => void run(onApply)}
            >
              {busy ? <Loader2 className="animate-spin" /> : <Check size={14} />}
              {t("themeGallery.apply", { defaultValue: "Apply" })}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

interface CatalogThemeRowProps {
  current: boolean
  favorite: boolean
  onApply: () => void
  onSelect: (theme: CatalogTheme) => void
  onToggleFavorite: (themeId: string) => void
  selected: boolean
  theme: CatalogTheme
}

const ROW_SWATCHES = ["red", "green", "yellow", "blue", "magenta", "cyan"] as const

const CatalogThemeRow = React.memo(function CatalogThemeRow({
  current,
  favorite,
  onApply,
  onSelect,
  onToggleFavorite,
  selected,
  theme,
}: CatalogThemeRowProps) {
  const { t } = useTranslation()
  return (
    <div
      role="option"
      aria-selected={selected}
      onClick={() => onSelect(theme)}
      onDoubleClick={onApply}
      className={cn(
        "group flex cursor-default items-center gap-2.5 rounded-md px-2 py-1.5 text-sm",
        selected ? "bg-accent ring-primary ring-1 ring-inset" : "hover:bg-accent/60"
      )}
    >
      <span
        className="flex h-5 shrink-0 items-center gap-0.5 rounded border border-white/10 px-1"
        style={{ background: theme.terminal.background }}
      >
        {ROW_SWATCHES.map((key) => (
          <span
            key={key}
            className="size-2 rounded-full"
            style={{ background: theme.terminal[key] }}
          />
        ))}
      </span>
      <span className="min-w-0 flex-1 truncate">{theme.name}</span>
      {current && <Check size={14} className="text-primary shrink-0" />}
      <button
        type="button"
        tabIndex={-1}
        aria-pressed={favorite}
        aria-label={
          favorite
            ? t("themeGallery.unfavorite", { defaultValue: "Remove from favorites" })
            : t("themeGallery.favorite", { defaultValue: "Add to favorites" })
        }
        onClick={(event) => {
          event.stopPropagation()
          onToggleFavorite(theme.id)
        }}
        onDoubleClick={(event) => event.stopPropagation()}
        className={cn(
          "text-muted-foreground hover:text-foreground shrink-0 rounded p-0.5",
          !favorite && "opacity-0 group-hover:opacity-100 group-aria-selected:opacity-100"
        )}
      >
        <Star size={14} className={favorite ? "fill-current text-yellow-500" : undefined} />
      </button>
    </div>
  )
})
