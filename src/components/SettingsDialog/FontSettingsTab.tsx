import React from "react"
import { invoke } from "@tauri-apps/api/core"
import { Copy, Eye, ListRestart, Type } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import {
  DEFAULT_SCROLLBACK_LINES,
  MAX_EXPLICIT_SCROLLBACK_LINES,
  UNLIMITED_SCROLLBACK_BUFFER,
  UNLIMITED_SCROLLBACK_SENTINEL,
  isUnlimitedScrollback,
} from "@/lib/scrollback"
import { toast } from "@/hooks/use-toast"
import { FISH_SHELL_INTEGRATION, POSIX_SHELL_INTEGRATION } from "@/lib/remoteShellIntegration"
import { cn } from "@/lib/utils"
import { CursorStylePicker } from "@/components/SettingsDialog/CursorStylePicker"
import { SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import {
  getDetectedPlatform,
  TERMINAL_LETTER_SPACING_RANGE,
  TERMINAL_LINE_HEIGHT_RANGE,
  type TerminalRenderer,
  HIDDEN_RENDERER_RELEASE_SECS_OPTIONS,
} from "@/contexts/ConfigContext"

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
  macOptionIsMeta: boolean
  setMacOptionIsMeta: (value: boolean) => void
  confirmMultilinePaste: boolean
  setConfirmMultilinePaste: (value: boolean) => void
  copyOnSelect: boolean
  setCopyOnSelect: (value: boolean) => void
  commandMarks: boolean
  setCommandMarks: (value: boolean) => void
  localShellIntegration: boolean
  setLocalShellIntegration: (value: boolean) => void
  rightClickPaste: boolean
  setRightClickPaste: (value: boolean) => void
  lineHeight: number
  setLineHeight: (value: number) => void
  letterSpacing: number
  setLetterSpacing: (value: number) => void
  fontLoadError: string | null
  handleFontSave: () => Promise<void>
  savingFont?: boolean
  loadingFonts: boolean
  scrollbackLines: number
  terminalRenderer: TerminalRenderer
  hiddenRendererReleaseSecs: number
  terminalPaddingLeftPx: number
  terminalPaddingRightPx: number
  terminalPaddingBottomPx: number
  setFontFamily: React.Dispatch<React.SetStateAction<string>>
  setFontSize: React.Dispatch<React.SetStateAction<number>>
  setCursorStyle: React.Dispatch<React.SetStateAction<"bar" | "block" | "underline">>
  setScrollbackLines: React.Dispatch<React.SetStateAction<number>>
  setTerminalRenderer: React.Dispatch<React.SetStateAction<TerminalRenderer>>
  setHiddenRendererReleaseSecs: React.Dispatch<React.SetStateAction<number>>
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
  macOptionIsMeta,
  setMacOptionIsMeta,
  confirmMultilinePaste,
  setConfirmMultilinePaste,
  copyOnSelect,
  setCopyOnSelect,
  commandMarks,
  setCommandMarks,
  localShellIntegration,
  setLocalShellIntegration,
  rightClickPaste,
  setRightClickPaste,
  lineHeight,
  setLineHeight,
  letterSpacing,
  setLetterSpacing,
  fontLoadError,
  handleFontSave,
  savingFont = false,
  loadingFonts,
  scrollbackLines,
  terminalRenderer,
  hiddenRendererReleaseSecs,
  terminalPaddingLeftPx,
  terminalPaddingRightPx,
  terminalPaddingBottomPx,
  setFontFamily,
  setFontSize,
  setCursorStyle,
  setScrollbackLines,
  setTerminalRenderer,
  setHiddenRendererReleaseSecs,
  setTerminalPaddingLeftPx,
  setTerminalPaddingRightPx,
  setTerminalPaddingBottomPx,
  systemFonts,
  fontSizeOptions,
}) => {
  const { t } = useTranslation()
  const copyShellIntegration = (script: string) => {
    invoke("plugin:clipboard-manager|write_text", { text: script })
      .then(() => toast({ title: t("fontSettings.integrationCopied") }))
      .catch((error) => {
        console.error("Failed to copy shell integration script:", error)
        toast({ title: t("terminalContext.copyFailedTitle"), variant: "destructive" })
      })
  }
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
                {t("fontSettings.rendererCanvas", { defaultValue: "Canvas" })}
              </option>
            </Select>
            <p className="text-muted-foreground mt-1.5 text-xs">
              {t("fontSettings.rendererDesc", {
                defaultValue:
                  "WebGL draws each terminal on one canvas and uses far less memory; Canvas keeps four full-size layers per terminal and is the fallback when WebGL is unavailable.",
              })}
            </p>
          </div>

          <div>
            <Label htmlFor="hidden-renderer-release" className="mb-2 block">
              {t("fontSettings.rendererRelease", {
                defaultValue: "Release renderer of hidden terminals",
              })}
            </Label>
            <Select
              id="hidden-renderer-release"
              value={String(hiddenRendererReleaseSecs)}
              onChange={(event) => setHiddenRendererReleaseSecs(Number(event.target.value))}
            >
              {HIDDEN_RENDERER_RELEASE_SECS_OPTIONS.map((secs) => (
                <option key={secs} value={secs}>
                  {secs === 0
                    ? t("fontSettings.rendererReleaseNever", { defaultValue: "Never" })
                    : secs < 60
                      ? t("fontSettings.rendererReleaseSeconds", {
                          count: secs,
                          defaultValue: "After {{count}} seconds",
                        })
                      : t("fontSettings.rendererReleaseMinutes", {
                          count: secs / 60,
                          defaultValue: "After {{count}} minutes",
                        })}
                </option>
              ))}
            </Select>
            <p className="text-muted-foreground mt-1.5 text-xs">
              {t("fontSettings.rendererReleaseDesc", {
                defaultValue:
                  "A terminal out of sight this long gives back its renderer's memory and loads it again when shown. Never switches tabs fastest, but every background tab keeps its renderer's memory, and with WebGL more than about 16 open terminals exceed WebKit's limit on live contexts, so the oldest fall back to Canvas.",
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

          <div className="grid gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label htmlFor="terminal-line-height">{t("fontSettings.lineHeight")}</Label>
              <Input
                id="terminal-line-height"
                type="number"
                min={TERMINAL_LINE_HEIGHT_RANGE.min}
                max={TERMINAL_LINE_HEIGHT_RANGE.max}
                step={0.05}
                value={lineHeight}
                onChange={(e) => {
                  const value = parseFloat(e.target.value)
                  if (
                    !isNaN(value) &&
                    value >= TERMINAL_LINE_HEIGHT_RANGE.min &&
                    value <= TERMINAL_LINE_HEIGHT_RANGE.max
                  ) {
                    setLineHeight(Math.round(value * 100) / 100)
                  }
                }}
                className="h-8"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="terminal-letter-spacing">{t("fontSettings.letterSpacing")}</Label>
              <Input
                id="terminal-letter-spacing"
                type="number"
                min={TERMINAL_LETTER_SPACING_RANGE.min}
                max={TERMINAL_LETTER_SPACING_RANGE.max}
                step={1}
                value={letterSpacing}
                onChange={(e) => {
                  const value = parseInt(e.target.value, 10)
                  if (
                    !isNaN(value) &&
                    value >= TERMINAL_LETTER_SPACING_RANGE.min &&
                    value <= TERMINAL_LETTER_SPACING_RANGE.max
                  ) {
                    setLetterSpacing(value)
                  }
                }}
                className="h-8"
              />
            </div>
            <p className="text-muted-foreground text-xs sm:col-span-2">
              {t("fontSettings.spacingDesc")}
            </p>
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

          {getDetectedPlatform() === "macos" && (
            <div className="flex items-start justify-between gap-4">
              <div>
                <Label htmlFor="terminal-option-is-meta" className="mb-1 block">
                  {t("fontSettings.macOptionIsMeta")}
                </Label>
                <p className="text-muted-foreground text-xs leading-5">
                  {t("fontSettings.macOptionIsMetaDesc")}
                </p>
              </div>
              <Switch
                id="terminal-option-is-meta"
                checked={macOptionIsMeta}
                onCheckedChange={setMacOptionIsMeta}
              />
            </div>
          )}

          <div className="flex items-start justify-between gap-4">
            <div>
              <Label htmlFor="terminal-confirm-multiline-paste" className="mb-1 block">
                {t("fontSettings.confirmMultilinePaste")}
              </Label>
              <p className="text-muted-foreground text-xs leading-5">
                {t("fontSettings.confirmMultilinePasteDesc")}
              </p>
            </div>
            <Switch
              id="terminal-confirm-multiline-paste"
              checked={confirmMultilinePaste}
              onCheckedChange={setConfirmMultilinePaste}
            />
          </div>

          <div className="flex items-start justify-between gap-4">
            <div>
              <Label htmlFor="terminal-copy-on-select" className="mb-1 block">
                {t("fontSettings.copyOnSelect")}
              </Label>
              <p className="text-muted-foreground text-xs leading-5">
                {t("fontSettings.copyOnSelectDesc")}
              </p>
            </div>
            <Switch
              id="terminal-copy-on-select"
              checked={copyOnSelect}
              onCheckedChange={setCopyOnSelect}
            />
          </div>

          <div className="flex items-start justify-between gap-4">
            <div>
              <Label htmlFor="terminal-right-click-paste" className="mb-1 block">
                {t("fontSettings.rightClickPaste")}
              </Label>
              <p className="text-muted-foreground text-xs leading-5">
                {t("fontSettings.rightClickPasteDesc")}
              </p>
            </div>
            <Switch
              id="terminal-right-click-paste"
              checked={rightClickPaste}
              onCheckedChange={setRightClickPaste}
            />
          </div>

          <div className="flex items-start justify-between gap-4">
            <div>
              <Label htmlFor="terminal-command-marks" className="mb-1 block">
                {t("fontSettings.commandMarks")}
              </Label>
              <p className="text-muted-foreground text-xs leading-5">
                {t("fontSettings.commandMarksDesc")}
              </p>
              <p className="text-muted-foreground mt-2 text-xs leading-5">
                {t("fontSettings.commandMarksRemote")}
              </p>
              <div className="mt-2 flex flex-wrap gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="xs"
                  onClick={() => copyShellIntegration(POSIX_SHELL_INTEGRATION)}
                >
                  <Copy aria-hidden="true" />
                  {t("fontSettings.copyPosixIntegration")}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="xs"
                  onClick={() => copyShellIntegration(FISH_SHELL_INTEGRATION)}
                >
                  <Copy aria-hidden="true" />
                  {t("fontSettings.copyFishIntegration")}
                </Button>
              </div>
            </div>
            <Switch
              id="terminal-command-marks"
              checked={commandMarks}
              onCheckedChange={setCommandMarks}
            />
          </div>

          {getDetectedPlatform() !== "windows" && (
            <div className="flex items-start justify-between gap-4">
              <div>
                <Label htmlFor="terminal-local-shell-integration" className="mb-1 block">
                  {t("fontSettings.localShellIntegration")}
                </Label>
                <p className="text-muted-foreground text-xs leading-5">
                  {t("fontSettings.localShellIntegrationDesc")}
                </p>
              </div>
              <Switch
                id="terminal-local-shell-integration"
                checked={localShellIntegration}
                disabled={!commandMarks}
                onCheckedChange={setLocalShellIntegration}
              />
            </div>
          )}

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
                    max: UNLIMITED_SCROLLBACK_BUFFER.toLocaleString(),
                    defaultValue:
                      "Unlimited is not truly unlimited: history keeps growing up to {{max}} lines and is only freed when you clear the screen. Memory per line grows with the window width; at 120 columns, 1 million lines take about 1.5 GB. All tabs share one interface process, and if their total memory goes past the limit the system allows it (about 16 GB on machines with plenty of RAM, less on smaller ones), the window may crash and every tab loses its scrollback. To keep output long term, turn on Terminal session logging.",
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
              style={{
                fontFamily,
                fontSize: `${fontSize}px`,
                lineHeight,
                letterSpacing: `${letterSpacing}px`,
              }}
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

        <Button onClick={handleFontSave} disabled={savingFont} className="w-full">
          {savingFont
            ? t("common.saving", { defaultValue: "Saving..." })
            : t("fontSettings.save", { defaultValue: "Save" })}
        </Button>
      </div>
    </ScrollArea>
  )
}
