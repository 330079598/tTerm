import React, { useEffect, useRef, useCallback } from "react"
import { Terminal } from "@xterm/xterm"
import { FitAddon } from "@xterm/addon-fit"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import "@xterm/xterm/css/xterm.css"

interface TerminalTabProps {
  tabId: string
  isActive: boolean
  onPidChange?: (pid: number) => void
}

export const TerminalTab: React.FC<TerminalTabProps> = ({ tabId, isActive, onPidChange }) => {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const initializedRef = useRef(false)

  const fitTerminal = useCallback(() => {
    if (!fitAddonRef.current || !termRef.current) return
    fitAddonRef.current.fit()
    const { rows, cols } = termRef.current
    invoke("resize_pty", { tabId, rows, cols }).catch(() => {})
  }, [tabId])

  useEffect(() => {
    const container = containerRef.current
    if (!container || initializedRef.current) return
    initializedRef.current = true

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: '"JetBrains Mono", "Fira Code", Menlo, Monaco, monospace',
      theme: {
        background: "transparent",
        foreground: "#e2e8f0",
        cursor: "#e2e8f0",
        selectionBackground: "rgba(226, 232, 240, 0.3)",
        black: "#1e293b",
        red: "#f87171",
        green: "#4ade80",
        yellow: "#facc15",
        blue: "#60a5fa",
        magenta: "#c084fc",
        cyan: "#22d3ee",
        white: "#e2e8f0",
        brightBlack: "#475569",
        brightRed: "#fca5a5",
        brightGreen: "#86efac",
        brightYellow: "#fde047",
        brightBlue: "#93c5fd",
        brightMagenta: "#d8b4fe",
        brightCyan: "#67e8f9",
        brightWhite: "#f8fafc",
      },
      allowTransparency: true,
    })

    const fitAddon = new FitAddon()
    term.loadAddon(fitAddon)
    term.loadAddon(new WebLinksAddon())
    termRef.current = term
    fitAddonRef.current = fitAddon

    // Mount xterm into the container and perform initial fit
    term.open(container)
    fitAddon.fit()

    // Forward user keystrokes to the backend PTY
    term.onData((data) => {
      invoke("write_pty", { tabId, data }).catch(() => {})
    })

    let unlistenOutput: (() => void) | null = null
    let unlistenExit: (() => void) | null = null

    // Register event listeners before spawning the PTY so no output is lost
    Promise.all([
      listen<string>(`pty-output-${tabId}`, (event) => {
        term.write(event.payload)
      }),
      listen(`pty-exit-${tabId}`, () => {
        term.writeln("\r\n\x1b[33m[Process exited]\x1b[0m")
      }),
    ])
      .then(([unOut, unExit]) => {
        unlistenOutput = unOut
        unlistenExit = unExit
        return invoke<number>("create_pty", { tabId, rows: term.rows, cols: term.cols })
      })
      .then((pid) => {
        onPidChange?.(pid)
      })
      .catch((e) => {
        term.writeln(`\x1b[31mFailed to start terminal: ${e}\x1b[0m`)
      })

    // Resize the PTY whenever the container dimensions change
    const resizeObserver = new ResizeObserver(() => fitTerminal())
    resizeObserver.observe(container)

    return () => {
      resizeObserver.disconnect()
      unlistenOutput?.()
      unlistenExit?.()
      invoke("kill_pty", { tabId }).catch(() => {})
      term.dispose()
    }
  }, [tabId, onPidChange, fitTerminal])

  // Re-fit when this tab is brought into view
  useEffect(() => {
    if (isActive) {
      setTimeout(fitTerminal, 50)
    }
  }, [isActive, fitTerminal])

  return <div ref={containerRef} style={{ width: "100%", height: "100%", overflow: "hidden" }} />
}
