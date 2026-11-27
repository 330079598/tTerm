import "@/i18n/config"
import "@/components/TerminalTab.css"

import { FitAddon } from "@xterm/addon-fit"
import { Terminal as XTerminal } from "@xterm/xterm"
import { useEffect, useRef, type CSSProperties } from "react"

import { useConfig } from "@/contexts/ConfigContext"
import type { TerminalRenderer } from "@/lib/appConfig"
import { getPresetTheme } from "@/lib/themeDefinitions"
import { firstScreenTerminalOutput } from "@/parity/screens/firstScreen/fixture"

/**
 * Loads the renderer the production tab would use. WebGL is the default and
 * falls back to canvas; the DOM renderer is never chosen for the screenshot
 * because it draws through the page font instead of the terminal font.
 */
async function loadRenderer(
  term: XTerminal,
  renderer: TerminalRenderer,
  isCancelled: () => boolean
) {
  if (renderer === "webgl") {
    try {
      const { WebglAddon } = await import("@xterm/addon-webgl")
      if (isCancelled()) return
      term.loadAddon(new WebglAddon())
      return
    } catch (error) {
      console.warn("Parity: WebGL is unavailable, using the canvas renderer", error)
    }
  }
  try {
    const { CanvasAddon } = await import("@xterm/addon-canvas")
    if (isCancelled()) return
    term.loadAddon(new CanvasAddon())
  } catch (error) {
    console.error("Parity: the canvas renderer is unavailable", error)
  }
}

/**
 * The production xterm surface with synthetic output. It uses the resolved
 * configuration (font, line height, cursor, padding and renderer) and the
 * production stylesheet, and starts no shell, PTY or SSH process.
 */
export function FirstScreenTerminal({ onReady }: { onReady?: () => void }) {
  const { config, isLoaded } = useConfig()
  const surfaceRef = useRef<HTMLDivElement | null>(null)
  const hostRef = useRef<HTMLDivElement | null>(null)
  const {
    font_family: fontFamily,
    font_size: fontSize,
    terminal_line_height: lineHeight,
    terminal_letter_spacing: letterSpacing,
    terminal_renderer: renderer,
    terminal_padding_left_px: paddingLeft,
    terminal_padding_right_px: paddingRight,
    terminal_padding_bottom_px: paddingBottom,
    scrollback_lines: scrollback,
    cursor_style: cursorStyle,
  } = config

  useEffect(() => {
    if (!isLoaded) return
    const surface = surfaceRef.current
    const host = hostRef.current
    if (!surface || !host) return

    const themeId = document.documentElement.dataset.theme === "light" ? "light" : "default"
    const palette = getPresetTheme(themeId)?.terminal
    if (!palette) return

    let cancelled = false
    let terminal: XTerminal | null = null
    let fitAddon: FitAddon | null = null

    void (async () => {
      // The renderer and the cell measurement must see the font the config
      // names, not the fallback the page had before the font finished loading.
      await document.fonts.ready
      if (cancelled) return

      const term = new XTerminal({
        cursorBlink: true,
        cursorStyle,
        scrollback,
        fontSize,
        fontFamily,
        fontWeight: "normal",
        fontWeightBold: "bold",
        letterSpacing,
        lineHeight,
        theme: { ...palette, overviewRulerBorder: "#00000000" },
        allowProposedApi: true,
        disableStdin: true,
        // Same 10 px gutter the production tab reserves for its scrollbar.
        overviewRuler: { width: 10 },
      })
      terminal = term
      const fit = new FitAddon()
      fitAddon = fit
      term.loadAddon(fit)
      term.open(host)
      await loadRenderer(term, renderer, () => cancelled)
      if (cancelled) return
      fit.fit()
      // The production tab focuses its terminal when it becomes active, so
      // the captured cursor belongs to the prompt line.
      term.focus()
      await new Promise<void>((resolve) => term.write(firstScreenTerminalOutput, resolve))
      if (cancelled) return
      fit.fit()
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          if (!cancelled) onReady?.()
        })
      )
    })()

    return () => {
      cancelled = true
      fitAddon?.dispose()
      terminal?.dispose()
    }
  }, [
    cursorStyle,
    fontFamily,
    fontSize,
    isLoaded,
    letterSpacing,
    lineHeight,
    onReady,
    renderer,
    scrollback,
  ])

  return (
    <div
      ref={surfaceRef}
      className="terminal-surface"
      data-parity="workspace.first-screen.terminal"
      style={
        {
          position: "absolute",
          inset: 0,
          "--terminal-padding-left": `${paddingLeft}px`,
          "--terminal-padding-right": `${paddingRight}px`,
          "--terminal-padding-bottom": `${paddingBottom}px`,
        } as CSSProperties
      }
    >
      <div ref={hostRef} className="terminal-host" style={{ width: "100%", height: "100%" }} />
    </div>
  )
}
