import React, { useEffect, useRef, useState } from "react"
import { Check, ChevronDown, Loader2, Search } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Input } from "@/components/ui/input"
import { useTheme } from "@/contexts/ThemeContext"
import { isImeKeyEvent } from "@/lib/ime"
import { catalogThemeName, isCatalogThemeId } from "@/lib/themeCatalog"
import { cn } from "@/lib/utils"
import { PRESET_THEME_IDS, type TerminalPalette, type Theme } from "@/types/theme"

import {
  filterThemeGroups,
  type ThemePickerGroup,
} from "@/components/ConnectionDialog/connectionDialogUtils"

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
 * Picks a theme for a connection's terminal: a button that opens a searchable
 * list in place, since the theme library holds hundreds of themes.
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
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState("")
  const [highlightedKey, setHighlightedKey] = useState<string | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

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
  const groups: ThemePickerGroup[] = [
    {
      label: null,
      themes: [{ id: "", label: followAppLabel }],
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
      themes: group.themes.map((theme) => ({
        id: theme.id,
        label: themeLabel(theme),
        palette: theme.terminal,
      })),
    })),
  ]
  const visibleGroups = filterThemeGroups(groups, query)
  // The same theme can be in two groups (a favorite is in the library too).
  const entries = visibleGroups.flatMap((group) =>
    group.themes.map((theme) => ({ ...theme, key: `${group.label ?? ""}:${theme.id}` }))
  )
  const highlighted = entries.find((entry) => entry.key === highlightedKey) ?? entries[0]

  const selectedTheme = value ? getTheme(value) : undefined
  const selectedLabel = !value
    ? followAppLabel
    : selectedTheme
      ? themeLabel(selectedTheme)
      : isCatalogThemeId(value) && !catalogThemes
        ? catalogThemeName(value)
        : t("connection.terminalThemeMissing", { id: value })

  const close = (refocus: boolean) => {
    setOpen(false)
    setQuery("")
    setHighlightedKey(null)
    if (refocus) triggerRef.current?.focus()
  }

  const choose = (themeId: string) => {
    onChange(themeId)
    close(true)
  }

  // Clicking anywhere else closes the list.
  useEffect(() => {
    if (!open) return
    const handlePointerDown = (event: PointerEvent) => {
      if (containerRef.current?.contains(event.target as Node)) return
      setOpen(false)
      setQuery("")
      setHighlightedKey(null)
    }
    document.addEventListener("pointerdown", handlePointerDown)
    return () => document.removeEventListener("pointerdown", handlePointerDown)
  }, [open])

  const highlightedEntryKey = highlighted?.key
  useEffect(() => {
    if (!open || !highlightedEntryKey) return
    listRef.current
      ?.querySelector<HTMLElement>('[aria-selected="true"]')
      ?.scrollIntoView({ block: "nearest" })
  }, [highlightedEntryKey, open])

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (isImeKeyEvent(event.nativeEvent)) return
    if (event.key === "Escape") {
      // Closes the list, not the dialog around it.
      event.preventDefault()
      event.stopPropagation()
      close(true)
      return
    }
    if (event.key === "Enter") {
      // The field sits in a form; Enter must not submit it.
      event.preventDefault()
      if (highlighted) choose(highlighted.id)
      return
    }
    if (entries.length === 0) return
    const index = highlighted ? entries.indexOf(highlighted) : -1
    const last = entries.length - 1
    const target = {
      ArrowDown: Math.min(last, index + 1),
      ArrowUp: Math.max(0, index - 1),
      PageDown: Math.min(last, index + 10),
      PageUp: Math.max(0, index - 10),
    }[event.key]
    if (target !== undefined) {
      event.preventDefault()
      setHighlightedKey(entries[target].key)
    }
  }

  const openList = () => {
    // Start on the current choice so the arrow keys continue from it.
    const current = entries.find((entry) => entry.id === value)
    setHighlightedKey(current?.key ?? null)
    setOpen(true)
  }

  return (
    <div ref={containerRef}>
      <button
        ref={triggerRef}
        id={id}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => (open ? close(false) : openList())}
        className="border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full items-center gap-2 rounded-md border px-3 text-left text-sm shadow-xs outline-none focus-visible:ring-[3px]"
      >
        <Swatch palette={selectedTheme?.terminal} />
        <span className="min-w-0 flex-1 truncate">{selectedLabel}</span>
        <ChevronDown className="text-muted-foreground size-4 shrink-0" />
      </button>

      {open && (
        <div className="mt-1 rounded-md border shadow-sm" onKeyDown={handleKeyDown}>
          <div className="relative border-b p-1">
            <Search
              size={14}
              className="text-muted-foreground pointer-events-none absolute top-1/2 left-3.5 -translate-y-1/2"
            />
            <Input
              autoFocus
              value={query}
              onChange={(event) => {
                setQuery(event.target.value)
                setHighlightedKey(null)
              }}
              placeholder={t("themeGallery.search")}
              aria-label={t("themeGallery.search")}
              aria-controls={`${id}-list`}
              aria-activedescendant={highlighted ? `${id}-${highlighted.key}` : undefined}
              className="h-8 border-0 pl-8 shadow-none focus-visible:ring-0"
            />
          </div>
          <div
            ref={listRef}
            id={`${id}-list`}
            role="listbox"
            aria-label={t("connection.terminalTheme")}
            className="max-h-64 overflow-y-auto p-1"
          >
            {visibleGroups.map((group) => (
              <div key={group.label ?? ""} role="group" aria-label={group.label ?? undefined}>
                {group.label && (
                  <div className="text-muted-foreground px-2 pt-2 pb-1 text-xs font-medium">
                    {group.label}
                  </div>
                )}
                {group.themes.map((theme) => {
                  const key = `${group.label ?? ""}:${theme.id}`
                  return (
                    <div
                      key={key}
                      id={`${id}-${key}`}
                      role="option"
                      aria-selected={key === highlighted?.key}
                      onPointerMove={() => setHighlightedKey(key)}
                      onClick={() => choose(theme.id)}
                      className={cn(
                        "flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm",
                        key === highlighted?.key && "bg-muted"
                      )}
                    >
                      <Swatch palette={theme.palette} />
                      <span className="min-w-0 flex-1 truncate">{theme.label}</span>
                      {theme.id === value && <Check className="size-3.5 shrink-0" />}
                    </div>
                  )
                })}
              </div>
            ))}
            {entries.length === 0 && catalogThemes && (
              <p className="text-muted-foreground px-2 py-4 text-center text-sm">
                {t("themeGallery.noMatch")}
              </p>
            )}
            {!catalogThemes && (
              <div className="text-muted-foreground flex items-center gap-2 px-2 py-2 text-xs">
                <Loader2 className="size-3.5 animate-spin" />
                {t("themeGallery.loading")}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
