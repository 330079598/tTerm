import React from "react"
import { Eye, ListRestart, Type } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  DEFAULT_SCROLLBACK_LINES,
  MAX_EXPLICIT_SCROLLBACK_LINES,
  UNLIMITED_SCROLLBACK_SENTINEL,
  isUnlimitedScrollback,
} from "@/lib/scrollback"
import { cn } from "@/lib/utils"
import { CursorStylePicker } from "@/components/SettingsDialog/CursorStylePicker"
import { SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import type { TerminalRenderer } from "@/contexts/ConfigContext"

const SCROLLBACK_PRESETS = [1000, 5000, DEFAULT_SCROLLBACK_LINES, 50000, 100000] as const

const NERD_FONT_PATTERN =
  /nerd\s*font|nerdfont|nf\b|nerd|powerline|meslo\s+lg.*nerd|fira\s*code.*nerd|jetbrains.*nerd|hack.*nerd|iosevka.*nerd|cascadia.*nerd|roboto.*mono.*nerd|ubuntu.*mono.*nerd|source\s*code\s*pro.*nerd|dejavu.*sans.*mono.*nerd|liberation.*mono.*nerd|noto.*sans.*mono.*nerd/i

function isNerdFont(name: string): boolean {
  return NERD_FONT_PATTERN.test(name)
}

interface FontSettingsTabProps {
  fontFamily: string
  fontSize: number
  cursorStyle: "bar" | "block" | "underline"
  fontLoadError: string | null
  handleFontSave: () => Promise<void>
  loadingFonts: boolean
  scrollbackLines: number
  terminalRenderer: TerminalRenderer
  terminalPaddingLeftPx: number
  terminalPaddingRightPx: number
  terminalPaddingBottomPx: number
  setFontFamily: React.Dispatch<React.SetStateAction<string>>
  setFontSize: React.Dispatch<React.SetStateAction<number>>
  setCursorStyle: React.Dispatch<React.SetStateAction<"bar" | "block" | "underline">>
  setScrollbackLines: React.Dispatch<React.SetStateAction<number>>
  setTerminalRenderer: React.Dispatch<React.SetStateAction<TerminalRenderer>>
  setTerminalPaddingLeftPx: React.Dispatch<React.SetStateAction<number>>
  setTerminalPaddingRightPx: React.Dispatch<React.SetStateAction<number>>
  setTerminalPaddingBottomPx: React.Dispatch<React.SetStateAction<number>>
  systemFonts: string[]
  fontSizeOptions: number[]
}

export const FontSettingsTab: React.FC<FontSettingsTabProps> = ({
  fontFamily,
  fontSize,
  cursorStyle,
  fontLoadError,
  handleFontSave,
  loadingFonts,
  scrollbackLines,
  terminalRenderer,
  terminalPaddingLeftPx,
  terminalPaddingRightPx,
  terminalPaddingBottomPx,
  setFontFamily,
  setFontSize,
  setCursorStyle,
  setScrollbackLines,
  setTerminalRenderer,
  setTerminalPaddingLeftPx,
  setTerminalPaddingRightPx,
  setTerminalPaddingBottomPx,
  systemFonts,
  fontSizeOptions,
}) => {
  const { t } = useTranslation()
  const [fontSearchQuery, setFontSearchQuery] = React.useState("")
  const [showNerdFontsOnly, setShowNerdFontsOnly] = React.useState(false)

  const nerdFontCount = React.useMemo(
    () => systemFonts.filter((f) => isNerdFont(f)).length,
    [systemFonts]
  )

  const filteredFonts = React.useMemo(() => {
    let fonts = systemFonts
    if (fontSearchQuery.trim()) {
      const query = fontSearchQuery.toLowerCase()
      fonts = fonts.filter((font) => font.toLowerCase().includes(query))
    }
    if (showNerdFontsOnly) {
      fonts = fonts.filter((font) => isNerdFont(font))
    }
    return fonts
  }, [systemFonts, fontSearchQuery, showNerdFontsOnly])

  const terminalPaddingFields = [
    {
      id: "terminal-padding-left",
      label: t("fontSettings.terminalPaddingLeft", { defaultValue: "Left" }),
      value: terminalPaddingLeftPx,
      setter: setTerminalPaddingLeftPx,
    },
    {
      id: "terminal-padding-right",
      label: t("fontSettings.terminalPaddingRight", { defaultValue: "Right" }),
      value: terminalPaddingRightPx,
      setter: setTerminalPaddingRightPx,
    },
    {
      id: "terminal-padding-bottom",
      label: t("fontSettings.terminalPaddingBottom", { defaultValue: "Bottom" }),
      value: terminalPaddingBottomPx,
      setter: setTerminalPaddingBottomPx,
    },
  ]

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<Type size={16} />}
          title={t("fontSettings.textRendering", { defaultValue: "Text rendering" })}
          description={t("fontSettings.textRenderingDesc", {
            defaultValue: "Choose the typeface and size used by terminal sessions.",
          })}
        >
          <div>
            <Label htmlFor="terminal-renderer" className="mb-2 block">
              {t("fontSettings.renderer", { defaultValue: "Terminal renderer" })}
            </Label>
            <Select
              id="terminal-renderer"
              value={terminalRenderer}
              onChange={(event) => setTerminalRenderer(event.target.value as TerminalRenderer)}
            >
              <option value="webgl">
                {t("fontSettings.rendererWebgl", { defaultValue: "WebGL (recommended)" })}
              </option>
              <option value="canvas">
                {t("fontSettings.rendererCanvas", { defaultValue: "Canvas (lower memory)" })}
              </option>
            </Select>
            <p className="text-muted-foreground mt-1.5 text-xs">
              {t("fontSettings.rendererDesc", {
                defaultValue: "WebGL is faster for heavy output; Canvas can use less GPU memory.",
              })}
            </p>
          </div>

          <div>
            <Label className="mb-2 block">{t("fontSettings.fontSize")}</Label>
            <div className="flex flex-wrap gap-1.5">
              {fontSizeOptions.map((size) => (
                <Button
                  key={size}
                  type="button"
                  variant={fontSize === size ? "default" : "outline"}
                  size="xs"
                  onClick={() => setFontSize(size)}
                  className={cn("min-w-[2.25rem]", fontSize !== size && "text-muted-foreground")}
                >
                  {size}
                </Button>
              ))}
              <Input
                type="number"
                min={6}
                max={72}
                value={fontSize}
                onChange={(e) => {
                  const value = parseInt(e.target.value)
                  if (!isNaN(value) && value >= 6 && value <= 72) setFontSize(value)
                }}
                className="h-7 w-16 px-2 text-xs"
              />
            </div>
          </div>

          <div>
            <Label className="mb-2 block">{t("fontSettings.fontFamily")}</Label>
            <Input
              type="text"
              value={fontFamily}
              onChange={(e) => setFontFamily(e.target.value)}
              placeholder={t("fontSettings.customFont")}
              className="mb-2"
            />
            {nerdFontCount > 0 && !isNerdFont(fontFamily) && (
              <p className="mb-2 text-xs text-amber-600 dark:text-amber-400">
                {t("fontSettings.nerdFontWarning", {
                  defaultValue:
                    "Current font may not support Nerd Font characters (used by powerlevel10k, starship, etc.). Select a font with the NF badge below.",
                })}
              </p>
            )}

            {loadingFonts ? (
              <p className="text-muted-foreground text-xs">{t("fontSettings.loadingFonts")}</p>
            ) : (
              <>
                {fontLoadError ? (
                  <p className="mb-2 text-xs text-amber-600 dark:text-amber-400">{fontLoadError}</p>
                ) : null}
                <Input
                  type="text"
                  value={fontSearchQuery}
                  onChange={(e) => setFontSearchQuery(e.target.value)}
                  placeholder={t("fontSettings.searchFonts")}
                  className="mb-2"
                />
                {nerdFontCount > 0 && (
                  <button
                    type="button"
                    onClick={() => setShowNerdFontsOnly(!showNerdFontsOnly)}
                    className={cn(
                      "mb-2 inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors",
                      showNerdFontsOnly
                        ? "bg-primary text-primary-foreground"
                        : "bg-secondary text-secondary-foreground hover:bg-secondary/80"
                    )}
                  >
                    Nerd Font{" "}
                    {showNerdFontsOnly ? `(${filteredFonts.length})` : `(${nerdFontCount})`}
                  </button>
                )}
                <ScrollArea className="border-border h-48 rounded border">
                  <div className="p-1">
                    {systemFonts.length === 0 ? (
                      <p className="text-muted-foreground px-2 py-4 text-center text-xs">
                        {t("fontSettings.noFontsFound")}
                      </p>
                    ) : filteredFonts.length === 0 ? (
                      <p className="text-muted-foreground px-2 py-4 text-center text-xs">
                        {t("fontSettings.noMatchingFonts")}
                      </p>
                    ) : (
                      filteredFonts.map((font) => (
                        <button
                          key={font}
                          onClick={() => setFontFamily(`"${font}", monospace`)}
                          className={cn(
                            "hover:bg-accent flex w-full items-center justify-between rounded px-3 py-1.5 text-left text-sm transition-colors",
                            fontFamily.includes(font)
                              ? "bg-accent text-foreground"
                              : "text-muted-foreground"
                          )}
                          style={{ fontFamily: font }}
                        >
                          <span>{font}</span>
                          {isNerdFont(font) && (
                            <span className="ml-2 shrink-0 rounded bg-emerald-500/15 px-1.5 py-0.5 text-[10px] font-medium text-emerald-600 dark:text-emerald-400">
                              NF
                            </span>
                          )}
                        </button>
                      ))
                    )}
                  </div>
                </ScrollArea>
              </>
            )}
          </div>
        </SettingsSection>

        <SettingsSection
          icon={<ListRestart size={16} />}
          title={t("fontSettings.behavior", { defaultValue: "Terminal behavior" })}
          description={t("fontSettings.behaviorDesc", {
            defaultValue: "Tune cursor, history, and terminal surface spacing.",
          })}
        >
          <div>
            <Label className="mb-2 block">
              {t("fontSettings.cursorStyle", { defaultValue: "Cursor Style" })}
            </Label>
            <CursorStylePicker value={cursorStyle} onChange={setCursorStyle} />
          </div>

          <div>
            <Label className="mb-2 block">
              {t("fontSettings.scrollbackLines", { defaultValue: "Scrollback Lines" })}
            </Label>
            <div className="space-y-2">
              <div className="flex gap-2">
                <Input
                  type="number"
                  min={0}
                  max={MAX_EXPLICIT_SCROLLBACK_LINES}
                  value={scrollbackLines}
                  onChange={(e) => {
                    const value = parseInt(e.target.value, 10)
                    if (
                      !isNaN(value) &&
                      value >= UNLIMITED_SCROLLBACK_SENTINEL &&
                      value <= MAX_EXPLICIT_SCROLLBACK_LINES
                    ) {
                      setScrollbackLines(value)
                    }
                  }}
                  className="flex-1"
                  placeholder={t("fontSettings.scrollbackLinesPlaceholder", {
                    defaultValue: "Default 10000, or pick a preset",
                  })}
                />
              </div>
              <p className="text-muted-foreground text-xs">
                {t("fontSettings.scrollbackLinesDesc", {
                  defaultValue:
                    "Default: 10,000 lines. Unlimited history can use significant memory.",
                })}
              </p>
              {isUnlimitedScrollback(scrollbackLines) && (
                <p className="text-xs text-amber-600 dark:text-amber-500">
                  {t("fontSettings.scrollbackUnlimitedWarning", {
                    defaultValue:
                      "Heavy output or many tabs may slow the app or use a lot of memory.",
                  })}
                </p>
              )}
              <div className="flex flex-wrap gap-1.5">
                <Button
                  type="button"
                  variant={isUnlimitedScrollback(scrollbackLines) ? "default" : "outline"}
                  size="xs"
                  onClick={() => setScrollbackLines(UNLIMITED_SCROLLBACK_SENTINEL)}
                  className={cn(
                    "min-w-[3.5rem]",
                    !isUnlimitedScrollback(scrollbackLines) && "text-muted-foreground"
                  )}
                >
                  {t("fontSettings.unlimited", { defaultValue: "Unlimited" })}
                </Button>
                {SCROLLBACK_PRESETS.map((lines) => (
                  <Button
                    key={lines}
                    type="button"
                    variant={scrollbackLines === lines ? "default" : "outline"}
                    size="xs"
                    onClick={() => setScrollbackLines(lines)}
                    className={cn(
                      "min-w-[3.5rem]",
                      scrollbackLines !== lines && "text-muted-foreground"
                    )}
                  >
                    {lines >= 1000 ? `${lines / 1000}k` : lines}
                  </Button>
                ))}
              </div>
            </div>
          </div>

          <div>
            <Label className="mb-2 block">
              {t("fontSettings.terminalPadding", { defaultValue: "Terminal Padding" })}
            </Label>
            <div className="grid gap-3 sm:grid-cols-3">
              {terminalPaddingFields.map((field) => (
                <div key={field.id} className="space-y-1.5">
                  <Label htmlFor={field.id} className="text-muted-foreground text-xs">
                    {field.label}
                  </Label>
                  <Input
                    id={field.id}
                    type="number"
                    min={0}
                    max={80}
                    value={field.value}
                    onChange={(e) => {
                      const value = parseInt(e.target.value)
                      if (!isNaN(value) && value >= 0 && value <= 80) field.setter(value)
                    }}
                    className="h-8"
                  />
                </div>
              ))}
            </div>
            <p className="text-muted-foreground mt-2 text-xs">
              {t("fontSettings.terminalPaddingDesc", {
                defaultValue:
                  "Adjust left, right, and bottom spacing in pixels. Bottom spacing lifts the last terminal line away from the window edge.",
              })}
            </p>
          </div>
        </SettingsSection>

        <SettingsSection
          icon={<Eye size={16} />}
          title={t("fontSettings.preview")}
          description={t("fontSettings.previewDesc", {
            defaultValue: "Check plain text and prompt symbol rendering before saving.",
          })}
        >
          <div>
            <Label className="mb-2 block">{t("fontSettings.preview")}</Label>
            <div
              className="bg-secondary text-foreground border-border rounded-lg border px-4 py-3"
              style={{ fontFamily, fontSize: `${fontSize}px` }}
            >
              The quick brown fox jumps over the lazy dog 0123456789
            </div>
          </div>

          <div>
            <Label className="mb-2 block">
              {t("fontSettings.nerdFontPreview", { defaultValue: "Nerd Font Preview" })}
            </Label>
            <div
              className="bg-secondary text-foreground border-border rounded-lg border px-4 py-3"
              style={{ fontFamily, fontSize: `${fontSize}px`, lineHeight: 1.6 }}
            >
              <div> 󰊢 main 󰁕 ~ </div>
              <div> 12:34:56 user@host ~/projects</div>
            </div>
          </div>
        </SettingsSection>

        <Button onClick={handleFontSave} className="w-full">
          {t("fontSettings.save")}
        </Button>
      </div>
    </ScrollArea>
  )
}
