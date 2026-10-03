import React, { useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"
import { FileUp, Import, Loader2 } from "lucide-react"

import { ThemeLivePreview } from "@/components/ThemeEditor/ThemeLivePreview"
import { ThemePreviewSwatches } from "@/components/ThemePreviewSwatches"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { useTheme } from "@/contexts/ThemeContext"
import { deriveThemeColors, terminalPaletteFromScheme } from "@/lib/themeDerivation"
import { parseThemeFile, ThemeImportError, type ThemeFileFormat } from "@/lib/themeImport"
import { cn } from "@/lib/utils"
import type { ColorsAndPalette, CustomTheme } from "@/types/theme"

const FORMAT_NAMES: Record<ThemeFileFormat, string> = {
  ghostty: "Ghostty",
  windowsTerminal: "Windows Terminal",
  iterm: "iTerm2",
  alacritty: "Alacritty",
  kitty: "kitty",
}

type ParsedThemes =
  | { ok: true; format: ThemeFileFormat; themes: Array<ColorsAndPalette & { name: string }> }
  | { ok: false; error: ThemeImportError | null }

interface ThemeImportDialogProps {
  onClose: () => void
  /** Called with the new themes, the one to show first. */
  onImported: (themes: CustomTheme[]) => Promise<void>
}

/** Brings over color schemes saved by Ghostty, Windows Terminal, iTerm2, Alacritty or kitty. */
export const ThemeImportDialog: React.FC<ThemeImportDialogProps> = ({ onClose, onImported }) => {
  const { t } = useTranslation()
  const { createCustomThemes } = useTheme()
  const [text, setText] = useState("")
  const [fileName, setFileName] = useState("")
  const [nameOverride, setNameOverride] = useState<string | null>(null)
  const [previewIndex, setPreviewIndex] = useState(0)
  const [saving, setSaving] = useState(false)
  const [saveFailed, setSaveFailed] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const parsed = useMemo((): ParsedThemes | null => {
    if (!text.trim()) return null
    try {
      const result = parseThemeFile(text, fileName)
      return {
        ok: true,
        format: result.format,
        themes: result.schemes.map((scheme) => {
          const terminal = terminalPaletteFromScheme(scheme)
          return { name: scheme.name, terminal, colors: deriveThemeColors(terminal) }
        }),
      }
    } catch (error) {
      return { ok: false, error: error instanceof ThemeImportError ? error : null }
    }
  }, [fileName, text])

  const themes = parsed?.ok ? parsed.themes : []
  const previewed = themes[Math.min(previewIndex, themes.length - 1)]
  const single = themes.length === 1
  const singleName = nameOverride ?? themes[0]?.name ?? ""

  const changeSource = (nextText: string, nextFileName: string) => {
    setText(nextText)
    setFileName(nextFileName)
    setNameOverride(null)
    setPreviewIndex(0)
    setSaveFailed(false)
  }

  const handleFile = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ""
    if (file) changeSource(await file.text(), file.name)
  }

  const errorMessage = (() => {
    if (!parsed || parsed.ok) return null
    const error = parsed.error
    if (error?.reason === "missingColors") {
      return t("themeImport.missingColors", {
        defaultValue: "This theme is missing colors: {{colors}}",
        colors: error.missing.join(", "),
      })
    }
    if (error?.reason === "invalidFile") {
      return t("themeImport.invalidFile", { defaultValue: "This file is not valid JSON." })
    }
    return t("themeImport.unknownFormat", {
      defaultValue:
        "This is not a theme tTerm can read. Use a Ghostty, Windows Terminal, iTerm2 (.itermcolors), Alacritty or kitty theme.",
    })
  })()

  const handleImport = async () => {
    if (!parsed?.ok || (single && !singleName.trim())) return
    setSaving(true)
    setSaveFailed(false)
    try {
      const format = FORMAT_NAMES[parsed.format]
      const now = Date.now()
      const created = await createCustomThemes(
        themes.map((theme) => ({
          name: single ? singleName.trim() : theme.name,
          description: t("themeImport.importedFrom", {
            defaultValue: "Imported from {{format}}",
            format,
          }),
          colors: theme.colors,
          terminal: theme.terminal,
          isCustom: true,
          createdAt: now,
          updatedAt: now,
        }))
      )
      await onImported(created)
      onClose()
    } catch (error) {
      console.error("Failed to import themes:", error)
      setSaveFailed(true)
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="flex max-h-[min(46rem,calc(100vh-2rem))] w-[min(60rem,calc(100vw-2rem))] flex-col gap-3 overflow-hidden sm:max-w-5xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Import size={16} />
            {t("themeImport.title", { defaultValue: "Import Theme" })}
          </DialogTitle>
        </DialogHeader>

        <div className="grid min-h-0 flex-1 gap-4 overflow-y-auto md:grid-cols-[minmax(0,1fr)_20rem]">
          <div className="min-w-0 space-y-3">
            <p className="text-muted-foreground text-sm">
              {t("themeImport.description", {
                defaultValue:
                  "Bring over a terminal color scheme from Ghostty, Windows Terminal, iTerm2, Alacritty or kitty. tTerm builds matching interface colors from it.",
              })}
            </p>
            <div className="flex flex-wrap items-center gap-2">
              <Button type="button" variant="outline" onClick={() => fileInputRef.current?.click()}>
                <FileUp size={14} />
                {t("themeImport.chooseFile", { defaultValue: "Choose file…" })}
              </Button>
              {fileName && (
                <span className="text-muted-foreground truncate text-xs">{fileName}</span>
              )}
              <input
                ref={fileInputRef}
                // No filter: Ghostty's theme files have no extension.
                type="file"
                className="hidden"
                onChange={(event) => void handleFile(event)}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="theme-import-text">
                {t("themeImport.pasteLabel", { defaultValue: "Or paste the theme here" })}
              </Label>
              <Textarea
                id="theme-import-text"
                value={text}
                onChange={(event) => changeSource(event.target.value, "")}
                spellCheck={false}
                placeholder={
                  "palette = 0=#21222c\npalette = 1=#ff5555\n…\nbackground = #282a36\nforeground = #f8f8f2"
                }
                className="h-40 resize-none font-mono text-xs"
              />
            </div>

            {errorMessage && <p className="text-destructive text-sm">{errorMessage}</p>}

            {parsed?.ok && (
              <div className="space-y-2">
                <p className="text-muted-foreground text-xs">
                  {t("themeImport.detected", {
                    defaultValue: "{{format}} theme",
                    format: FORMAT_NAMES[parsed.format],
                  })}
                  {!single &&
                    ` · ${t("themeImport.schemeCount", {
                      defaultValue: "{{count}} themes, all will be imported",
                      count: themes.length,
                    })}`}
                </p>
                {single ? (
                  <div className="space-y-1.5">
                    <Label htmlFor="theme-import-name">{t("themeEditor.name")}</Label>
                    <Input
                      id="theme-import-name"
                      value={singleName}
                      onChange={(event) => setNameOverride(event.target.value)}
                    />
                  </div>
                ) : (
                  <div className="grid max-h-48 gap-1 overflow-y-auto rounded-lg border p-1">
                    {themes.map((theme, index) => (
                      <button
                        key={`${theme.name}-${index}`}
                        type="button"
                        onClick={() => setPreviewIndex(index)}
                        className={cn(
                          "flex items-center justify-between gap-3 rounded-md px-2 py-1.5 text-left text-sm",
                          theme === previewed ? "bg-accent" : "hover:bg-accent/60"
                        )}
                      >
                        <span className="truncate">{theme.name}</span>
                        <ThemePreviewSwatches compact palette={theme.terminal} />
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>

          <div className="min-w-0">
            {previewed ? (
              <ThemeLivePreview colors={previewed.colors} terminal={previewed.terminal} />
            ) : (
              <div className="text-muted-foreground flex h-full min-h-48 items-center justify-center rounded-2xl border border-dashed p-6 text-center text-sm">
                {t("themeImport.previewEmpty", {
                  defaultValue: "The theme shows here once tTerm can read it.",
                })}
              </div>
            )}
          </div>
        </div>

        <div className="flex flex-wrap items-center justify-end gap-2 border-t pt-3">
          {saveFailed && (
            <span className="text-destructive mr-auto text-sm">
              {t("themeImport.saveFailed", { defaultValue: "Failed to save the theme." })}
            </span>
          )}
          <Button type="button" variant="outline" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            type="button"
            disabled={themes.length === 0 || (single && !singleName.trim()) || saving}
            onClick={() => void handleImport()}
          >
            {saving ? <Loader2 className="animate-spin" /> : <Import size={14} />}
            {single || themes.length === 0
              ? t("themeImport.importAndApply", { defaultValue: "Import and apply" })
              : t("themeImport.importAll", {
                  defaultValue: "Import {{count}} themes",
                  count: themes.length,
                })}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
