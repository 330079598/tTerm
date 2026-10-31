import type { TFunction } from "i18next"
import React, { useRef, useState } from "react"

import type { AppConfig, TerminalRenderer } from "@/lib/appConfig"
import type { useConfig } from "@/contexts/ConfigContext"
import type { useToast } from "@/hooks/use-toast"
import {
  cachedSystemFonts,
  normalizeScrollbackLines,
} from "@/components/SettingsDialog/settingsLoaders"
import { useSyncedState } from "@/hooks/useSyncedState"
import { toErrorMessage } from "@/lib/utils"

interface TerminalSettingsDraftOptions {
  config: AppConfig
  saveConfig: ReturnType<typeof useConfig>["saveConfig"]
  toast: ReturnType<typeof useToast>["toast"]
  t: TFunction
  isMountedRef: React.RefObject<boolean>
}

/**
 * The terminal tab's settings as edited, saved together, and the system's
 * fonts to pick from.
 */
export function useTerminalSettingsDraft({
  config,
  saveConfig,
  toast,
  t,
  isMountedRef,
}: TerminalSettingsDraftOptions) {
  const [fontFamily, setFontFamily] = useState(config.font_family)
  // Follows the zoom shortcuts while the settings are open.
  const [fontSize, setFontSize] = useSyncedState(config.font_size)
  const [lineHeight, setLineHeight] = useState(config.terminal_line_height)
  const [letterSpacing, setLetterSpacing] = useState(config.terminal_letter_spacing)
  const [cursorStyle, setCursorStyle] = useState(config.cursor_style)
  const [macOptionIsMeta, setMacOptionIsMeta] = useState(config.mac_option_is_meta)
  const [confirmMultilinePaste, setConfirmMultilinePaste] = useState(config.confirm_multiline_paste)
  const [copyOnSelect, setCopyOnSelect] = useState(config.copy_on_select)
  const [commandMarks, setCommandMarks] = useState(config.command_marks)
  const [localShellIntegration, setLocalShellIntegration] = useState(
    config.local_unix_shell_integration
  )
  const [rightClickPaste, setRightClickPaste] = useState(config.right_click_paste)
  const [osc52ClipboardWrite, setOsc52ClipboardWrite] = useState(config.osc52_clipboard_write)
  const [scrollbackLines, setScrollbackLines] = useState(() =>
    normalizeScrollbackLines(config.scrollback_lines)
  )
  const [terminalRenderer, setTerminalRenderer] = useState<TerminalRenderer>(
    config.terminal_renderer
  )
  const [hiddenRendererReleaseSecs, setHiddenRendererReleaseSecs] = useState(
    config.hidden_renderer_release_secs
  )
  const [terminalPaddingLeftPx, setTerminalPaddingLeftPx] = useState(
    config.terminal_padding_left_px
  )
  const [terminalPaddingRightPx, setTerminalPaddingRightPx] = useState(
    config.terminal_padding_right_px
  )
  const [terminalPaddingBottomPx, setTerminalPaddingBottomPx] = useState(
    config.terminal_padding_bottom_px
  )
  const [savingFont, setSavingFont] = useState(false)
  const savingFontRef = useRef(false)
  const [systemFonts, setSystemFonts] = useState<string[]>(() => cachedSystemFonts ?? [])
  const [fontsLoaded, setFontsLoaded] = useState(cachedSystemFonts !== null)
  const [fontLoadError, setFontLoadError] = useState<string | null>(null)

  const handleFontSave = async () => {
    if (savingFontRef.current) {
      return
    }
    savingFontRef.current = true
    setSavingFont(true)
    try {
      await saveConfig({
        font_family: fontFamily,
        font_size: fontSize,
        terminal_line_height: lineHeight,
        terminal_letter_spacing: letterSpacing,
        cursor_style: cursorStyle,
        mac_option_is_meta: macOptionIsMeta,
        confirm_multiline_paste: confirmMultilinePaste,
        copy_on_select: copyOnSelect,
        right_click_paste: rightClickPaste,
        osc52_clipboard_write: osc52ClipboardWrite,
        command_marks: commandMarks,
        local_unix_shell_integration: localShellIntegration,
        scrollback_lines: scrollbackLines,
        terminal_renderer: terminalRenderer,
        hidden_renderer_release_secs: hiddenRendererReleaseSecs,
        terminal_padding_left_px: terminalPaddingLeftPx,
        terminal_padding_right_px: terminalPaddingRightPx,
        terminal_padding_bottom_px: terminalPaddingBottomPx,
      })
      toast({
        title: t("fontSettings.saved", { defaultValue: "Settings saved" }),
        description: t("fontSettings.savedDesc", {
          defaultValue:
            "Terminal settings have been saved. New terminal tabs will use the updated configuration.",
        }),
        variant: "success",
      })
    } catch (error) {
      console.error("Failed to save font settings:", error)
      toast({
        title: t("fontSettings.saveFailed", { defaultValue: "Failed to save" }),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      savingFontRef.current = false
      if (isMountedRef.current) {
        setSavingFont(false)
      }
    }
  }

  return {
    fontFamily,
    setFontFamily,
    fontSize,
    setFontSize,
    lineHeight,
    setLineHeight,
    letterSpacing,
    setLetterSpacing,
    cursorStyle,
    setCursorStyle,
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
    scrollbackLines,
    setScrollbackLines,
    terminalRenderer,
    setTerminalRenderer,
    hiddenRendererReleaseSecs,
    setHiddenRendererReleaseSecs,
    terminalPaddingLeftPx,
    setTerminalPaddingLeftPx,
    terminalPaddingRightPx,
    setTerminalPaddingRightPx,
    terminalPaddingBottomPx,
    setTerminalPaddingBottomPx,
    savingFont,
    systemFonts,
    setSystemFonts,
    fontsLoaded,
    setFontsLoaded,
    fontLoadError,
    setFontLoadError,
    handleFontSave,
  }
}
