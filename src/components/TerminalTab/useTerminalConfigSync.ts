import { useEffect, useRef } from "react"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import type { AppConfig } from "@/lib/appConfig"
import type { useTerminalFit } from "@/components/TerminalTab/useTerminalFit"
import type { useTerminalTheme } from "@/components/TerminalTab/useTerminalTheme"
import { scrollbackForSetting } from "@/components/TerminalTab/scrollbackMemory"
import { isTransparentTerminalTheme } from "@/lib/terminalPalette"
import { safePreloadFont, updateCanvasFontHostFont } from "@/lib/canvasFontHost"

type UseTerminalConfigSyncOptions = Pick<
  TerminalTabRefs,
  "termRef" | "isActiveRef" | "commandMarksRef"
> & { config: AppConfig } & { sessionNonce: number } & Pick<
    ReturnType<typeof useTerminalFit>,
    "scheduleFitDuringResize" | "fitAndSyncPty"
  > &
  Pick<ReturnType<typeof useTerminalTheme>, "resolveTerminalTheme">

/** Applies changed settings (font, cursor, scrollback, padding, theme) to the live terminal. */
export function useTerminalConfigSync({
  termRef,
  isActiveRef,
  commandMarksRef,
  config,
  sessionNonce,
  scheduleFitDuringResize,
  fitAndSyncPty,
  resolveTerminalTheme,
}: UseTerminalConfigSyncOptions) {
  const lastAppliedFontRef = useRef<{
    family: string
    size: number
    lineHeight: number
    letterSpacing: number
  } | null>(null)

  useEffect(() => {
    const term = termRef.current
    if (!term) return

    term.options.cursorStyle = config.cursor_style
    term.options.macOptionIsMeta = config.mac_option_is_meta

    const lastFont = lastAppliedFontRef.current
    const fontChanged =
      !lastFont ||
      lastFont.family !== config.font_family ||
      lastFont.size !== config.font_size ||
      lastFont.lineHeight !== config.terminal_line_height ||
      lastFont.letterSpacing !== config.terminal_letter_spacing

    if (fontChanged) {
      lastAppliedFontRef.current = {
        family: config.font_family,
        size: config.font_size,
        lineHeight: config.terminal_line_height,
        letterSpacing: config.terminal_letter_spacing,
      }
      updateCanvasFontHostFont(config.font_family, config.font_size)
      term.options.fontFamily = config.font_family
      term.options.fontSize = config.font_size
      term.options.lineHeight = config.terminal_line_height
      term.options.letterSpacing = config.terminal_letter_spacing

      if (isActiveRef.current) {
        scheduleFitDuringResize()
        term.refresh(0, Math.max(0, term.rows - 1))
      }

      void safePreloadFont(config.font_size, config.font_family).then((loaded) => {
        if (loaded && isActiveRef.current) {
          scheduleFitDuringResize()
          term.refresh(0, Math.max(0, term.rows - 1))
        }
      })
    }
  }, [
    config.cursor_style,
    config.mac_option_is_meta,
    config.font_family,
    config.font_size,
    config.terminal_letter_spacing,
    config.terminal_line_height,
    isActiveRef,
    scheduleFitDuringResize,
    termRef,
  ])

  useEffect(() => {
    commandMarksRef.current?.setEnabled(config.command_marks)
  }, [commandMarksRef, config.command_marks, sessionNonce])

  useEffect(() => {
    const term = termRef.current
    if (!term) return

    term.options.scrollback = scrollbackForSetting(term, config.scrollback_lines)
  }, [config.scrollback_lines, sessionNonce, termRef])

  useEffect(() => {
    if (!isActiveRef.current) return

    fitAndSyncPty()
  }, [
    config.terminal_padding_bottom_px,
    config.terminal_padding_left_px,
    config.terminal_padding_right_px,
    fitAndSyncPty,
    isActiveRef,
  ])

  useEffect(() => {
    const term = termRef.current
    if (!term) return

    const theme = resolveTerminalTheme()
    term.options.allowTransparency = isTransparentTerminalTheme(theme)
    term.options.theme = theme
  }, [resolveTerminalTheme, sessionNonce, termRef])
}
