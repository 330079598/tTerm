import React from "react"
import { invoke } from "@tauri-apps/api/core"
import { Copy, Eye, ListRestart, Type } from "lucide-react"
import { useTranslation } from "react-i18next"

import { FontFamilyPicker } from "@/components/SettingsDialog/FontFamilyPicker"
import { ScrollbackLinesField } from "@/components/SettingsDialog/ScrollbackLinesField"
import { TerminalPaddingFields } from "@/components/SettingsDialog/TerminalPaddingFields"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import { toast } from "@/hooks/use-toast"
import { FISH_SHELL_INTEGRATION, POSIX_SHELL_INTEGRATION } from "@/lib/remoteShellIntegration"
import { cn } from "@/lib/utils"
import { CursorStylePicker } from "@/components/SettingsDialog/CursorStylePicker"
import { SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { getDetectedPlatform } from "@/lib/uiPlatform"
import {
  TERMINAL_LETTER_SPACING_RANGE,
  TERMINAL_LINE_HEIGHT_RANGE,
  type TerminalRenderer,
  HIDDEN_RENDERER_RELEASE_SECS_OPTIONS,
} from "@/lib/appConfig"

export interface FontSettingsTabProps {
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
  osc52ClipboardWrite: boolean
  setOsc52ClipboardWrite: (value: boolean) => void
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
  osc52ClipboardWrite,
  setOsc52ClipboardWrite,
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

          <FontFamilyPicker
            fontFamily={fontFamily}
            setFontFamily={setFontFamily}
            systemFonts={systemFonts}
            loadingFonts={loadingFonts}
            fontLoadError={fontLoadError}
          />
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
              <Label htmlFor="terminal-osc52-clipboard-write" className="mb-1 block">
                {t("fontSettings.osc52ClipboardWrite")}
              </Label>
              <p className="text-muted-foreground text-xs leading-5">
                {t("fontSettings.osc52ClipboardWriteDesc")}
              </p>
            </div>
            <Switch
              id="terminal-osc52-clipboard-write"
              checked={osc52ClipboardWrite}
              onCheckedChange={setOsc52ClipboardWrite}
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

          <ScrollbackLinesField
            scrollbackLines={scrollbackLines}
            setScrollbackLines={setScrollbackLines}
          />

          <TerminalPaddingFields
            terminalPaddingLeftPx={terminalPaddingLeftPx}
            terminalPaddingRightPx={terminalPaddingRightPx}
            terminalPaddingBottomPx={terminalPaddingBottomPx}
            setTerminalPaddingLeftPx={setTerminalPaddingLeftPx}
            setTerminalPaddingRightPx={setTerminalPaddingRightPx}
            setTerminalPaddingBottomPx={setTerminalPaddingBottomPx}
          />
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
