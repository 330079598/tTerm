import React, { useEffect, useRef } from "react"
import { FitAddon } from "@xterm/addon-fit"
import { Terminal } from "@xterm/xterm"

import type { TerminalPalette } from "@/types/theme"

interface TerminalPalettePreviewProps {
  className?: string
  /** Focus the preview to show its cursor; off where another control keeps the keyboard. */
  focus?: boolean
  palette: TerminalPalette
}

const PREVIEW_LINES = [
  "\x1b[1;92mstone@tterm-preview\x1b[0m:\x1b[1;94m~/Code/rust/tTerm\x1b[0m \x1b[95m(theme-lab)\x1b[0m",
  "$ ls --color=always",
  "\x1b[1;34msrc\x1b[0m  \x1b[1;34msrc-tauri\x1b[0m  \x1b[1;32mdeploy.sh\x1b[0m  \x1b[36mtheme-link\x1b[0m  README.md  Cargo.toml",
  "$ git diff --color=always -- src/components/ThemeEditor.tsx",
  "\x1b[1m\x1b[90mdiff --git a/src/components/ThemeEditor.tsx b/src/components/ThemeEditor.tsx\x1b[0m",
  "\x1b[90mindex 9f3c2aa..c7bd441 100644\x1b[0m",
  "\x1b[31m--- a/src/components/ThemeEditor.tsx\x1b[0m",
  "\x1b[32m+++ b/src/components/ThemeEditor.tsx\x1b[0m",
  "\x1b[96m@@ -124,2 +124,3 @@\x1b[0m",
  '\x1b[31m-const warning = "#f6c177"\x1b[0m',
  '\x1b[32m+const warning = "#ffd27a"\x1b[0m',
  '\x1b[32m+const prompt = "stone@tterm-preview"\x1b[0m',
  "\x1b[33mwarning\x1b[0m Host key changed for dev-box; verify the fingerprint before reconnecting.",
  "\x1b[31merror\x1b[0m Permission denied (publickey).",
  "\x1b[1;92mstone@tterm-preview\x1b[0m:\x1b[1;94m~/Code/rust/tTerm\x1b[0m \x1b[95m(theme-lab)\x1b[0m $ ",
] as const

function writePreview(term: Terminal) {
  // Reset through the write queue (ESC c): `term.reset()` takes effect at
  // once, before earlier queued writes, which would then pile up.
  term.write(`\x1bc${PREVIEW_LINES.join("\r\n")}`)
}

export const TerminalPalettePreview: React.FC<TerminalPalettePreviewProps> = ({
  className,
  focus = true,
  palette,
}) => {
  const containerRef = useRef<HTMLDivElement>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const resizeObserverRef = useRef<ResizeObserver | null>(null)
  const initialPaletteRef = useRef(palette)
  const initialFocusRef = useRef(focus)

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const term = new Terminal({
      allowTransparency: false,
      convertEol: true,
      cursorBlink: false,
      cursorStyle: "block",
      disableStdin: true,
      fontFamily:
        '"JetBrains Mono Nerd Font", "SFMono-Regular", "JetBrains Mono", "Cascadia Code", monospace',
      fontSize: 12,
      lineHeight: 1.15,
      rows: PREVIEW_LINES.length,
      theme: initialPaletteRef.current,
    })

    const fitAddon = new FitAddon()
    term.loadAddon(fitAddon)

    terminalRef.current = term
    fitAddonRef.current = fitAddon

    term.open(container)
    fitAddon.fit()
    writePreview(term)
    if (initialFocusRef.current) term.focus()

    const resizeObserver = new ResizeObserver(() => {
      const { cols, rows } = term
      fitAddonRef.current?.fit()
      // Lines written at the old size stay wrapped and scrolled; start over.
      if (term.cols !== cols || term.rows !== rows) writePreview(term)
    })
    resizeObserver.observe(container)
    resizeObserverRef.current = resizeObserver

    return () => {
      resizeObserver.disconnect()
      resizeObserverRef.current = null
      terminalRef.current = null
      fitAddonRef.current = null
      term.dispose()
    }
  }, [])

  useEffect(() => {
    const term = terminalRef.current
    if (!term) return

    term.options.theme = palette
    // Fit first: lines written at the old size would stay wrapped.
    fitAddonRef.current?.fit()
    writePreview(term)
    if (focus) term.focus()
  }, [focus, palette])

  return (
    <div
      ref={containerRef}
      className={className}
      style={{
        backgroundColor: palette.background,
        height: 280,
        overflow: "hidden",
        width: "100%",
      }}
    />
  )
}
