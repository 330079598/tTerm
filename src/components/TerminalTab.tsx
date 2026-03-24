import "@/components/TerminalTab.css"
import React, { useEffect, useRef, useCallback } from "react"
import { Terminal } from "@xterm/xterm"
import { FitAddon } from "@xterm/addon-fit"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { WebglAddon } from "@xterm/addon-webgl"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useConfig } from "@/contexts/ConfigContext"
import "@xterm/xterm/css/xterm.css"

interface TerminalTabProps {
  tabId: string
  isActive: boolean
  onPidChange?: (pid: number) => void
}

const FALLBACK_TERMINAL_BACKGROUND = "#111827"
const OUTPUT_FLUSH_FRAME_BYTES = 256 * 1024
const MAX_PENDING_OUTPUT_BYTES = 4 * 1024 * 1024
const RESIZE_PTY_COMMIT_DELAY_MS = 90
const TAB_ACTIVATE_REFIT_DELAY_MS = 32

export const TerminalTab: React.FC<TerminalTabProps> = ({ tabId, isActive, onPidChange }) => {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const resizeObserverRef = useRef<ResizeObserver | null>(null)
  const webglAddonRef = useRef<WebglAddon | null>(null)
  const resizeRafRef = useRef<number | null>(null)
  const resizeEndTimerRef = useRef<number | null>(null)
  const activateFitTimerRef = useRef<number | null>(null)
  const outputFlushRafRef = useRef<number | null>(null)
  const pendingOutputRef = useRef<string[]>([])
  const pendingOutputBytesRef = useRef(0)
  const lastPtySizeRef = useRef<{ rows: number; cols: number } | null>(null)
  const isActiveRef = useRef(isActive)
  const initializedRef = useRef(false)
  const { config } = useConfig()
  const initialFontFamily = useRef(config.font_family)
  const initialFontSize = useRef(config.font_size)

  useEffect(() => {
    isActiveRef.current = isActive
  }, [isActive])

  const resolveTerminalBackground = useCallback(() => {
    if (typeof window === "undefined") {
      return FALLBACK_TERMINAL_BACKGROUND
    }

    const color = window
      .getComputedStyle(document.documentElement)
      .getPropertyValue("--background")
      .trim()

    return color ? `hsl(${color})` : FALLBACK_TERMINAL_BACKGROUND
  }, [])

  const fitTerminalOnly = useCallback(() => {
    if (!fitAddonRef.current || !termRef.current) return
    fitAddonRef.current.fit()
  }, [])

  const syncPtySize = useCallback(
    (force = false) => {
      const term = termRef.current
      if (!term) return

      const nextSize = { rows: term.rows, cols: term.cols }
      const prevSize = lastPtySizeRef.current
      if (
        !force &&
        prevSize &&
        prevSize.rows === nextSize.rows &&
        prevSize.cols === nextSize.cols
      ) {
        return
      }

      lastPtySizeRef.current = nextSize
      invoke("resize_pty", { tabId, rows: nextSize.rows, cols: nextSize.cols }).catch(() => {})
    },
    [tabId]
  )

  const fitAndSyncPty = useCallback(
    (force = false) => {
      fitTerminalOnly()
      syncPtySize(force)
    },
    [fitTerminalOnly, syncPtySize]
  )

  const scheduleFitDuringResize = useCallback(() => {
    if (resizeRafRef.current !== null) return

    resizeRafRef.current = window.requestAnimationFrame(() => {
      resizeRafRef.current = null
      if (!isActiveRef.current) return
      fitTerminalOnly()
    })
  }, [fitTerminalOnly])

  const schedulePtyResizeCommit = useCallback(() => {
    if (resizeEndTimerRef.current !== null) {
      window.clearTimeout(resizeEndTimerRef.current)
    }

    resizeEndTimerRef.current = window.setTimeout(() => {
      resizeEndTimerRef.current = null
      if (!isActiveRef.current) return
      fitAndSyncPty()
    }, RESIZE_PTY_COMMIT_DELAY_MS)
  }, [fitAndSyncPty])

  const flushPendingOutput = useCallback(function flushPendingOutputNow() {
    const term = termRef.current
    if (!term || pendingOutputRef.current.length === 0) {
      return
    }

    let bytesWritten = 0
    const chunkGroup: string[] = []
    while (
      pendingOutputRef.current.length > 0 &&
      (bytesWritten < OUTPUT_FLUSH_FRAME_BYTES || chunkGroup.length === 0)
    ) {
      const nextChunk = pendingOutputRef.current.shift()
      if (!nextChunk) break
      chunkGroup.push(nextChunk)
      bytesWritten += nextChunk.length
    }

    pendingOutputBytesRef.current -= bytesWritten
    if (pendingOutputBytesRef.current < 0) {
      pendingOutputBytesRef.current = 0
    }

    term.write(chunkGroup.join(""))

    if (pendingOutputRef.current.length > 0 && outputFlushRafRef.current === null) {
      outputFlushRafRef.current = window.requestAnimationFrame(() => {
        outputFlushRafRef.current = null
        flushPendingOutputNow()
      })
    }
  }, [])

  const scheduleOutputFlush = useCallback(() => {
    if (outputFlushRafRef.current !== null) return

    outputFlushRafRef.current = window.requestAnimationFrame(() => {
      outputFlushRafRef.current = null
      flushPendingOutput()
    })
  }, [flushPendingOutput])

  const enqueueOutput = useCallback(
    (output: string) => {
      if (!output) return
      pendingOutputRef.current.push(output)
      pendingOutputBytesRef.current += output.length

      if (pendingOutputBytesRef.current >= MAX_PENDING_OUTPUT_BYTES) {
        flushPendingOutput()
        return
      }

      scheduleOutputFlush()
    },
    [flushPendingOutput, scheduleOutputFlush]
  )

  // Update font options when config changes (after terminal is initialized)
  useEffect(() => {
    const term = termRef.current
    if (!term) return

    term.options.fontFamily = config.font_family
    term.options.fontSize = config.font_size
    if (isActiveRef.current) {
      fitAndSyncPty()
    }
  }, [config.font_family, config.font_size, fitAndSyncPty])

  // Keep terminal background in sync with app theme while staying opaque for faster compositing.
  useEffect(() => {
    const term = termRef.current
    if (!term) return

    term.options.theme = {
      ...(term.options.theme ?? {}),
      background: resolveTerminalBackground(),
    }
  }, [config.theme, resolveTerminalBackground])

  useEffect(() => {
    const container = containerRef.current
    if (!container || initializedRef.current) return
    initializedRef.current = true

    const term = new Terminal({
      cursorBlink: true,
      fontSize: initialFontSize.current,
      fontFamily: initialFontFamily.current,
      theme: {
        background: resolveTerminalBackground(),
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
      allowTransparency: false,
    })

    const fitAddon = new FitAddon()
    term.loadAddon(fitAddon)
    term.loadAddon(new WebLinksAddon())
    try {
      const webglAddon = new WebglAddon()
      term.loadAddon(webglAddon)
      webglAddon.onContextLoss(() => {
        webglAddon.dispose()
        if (webglAddonRef.current === webglAddon) {
          webglAddonRef.current = null
        }
      })
      webglAddonRef.current = webglAddon
    } catch {
      // Fallback to the default renderer when WebGL is not available.
      webglAddonRef.current = null
    }

    termRef.current = term
    fitAddonRef.current = fitAddon

    // Mount xterm into the container and perform initial fit
    term.open(container)
    fitTerminalOnly()

    // Forward user keystrokes to the backend PTY
    term.onData((data) => {
      invoke("write_pty", { tabId, data }).catch(() => {})
    })

    let unlistenOutput: (() => void) | null = null
    let unlistenExit: (() => void) | null = null

    // Register event listeners before spawning the PTY so no output is lost
    Promise.all([
      listen<string>(`pty-output-${tabId}`, (event) => {
        enqueueOutput(event.payload)
      }),
      listen(`pty-exit-${tabId}`, () => {
        flushPendingOutput()
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

    // During live-resize we fit in RAF; PTY resize is committed once resizing settles.
    const resizeObserver = new ResizeObserver(() => {
      if (!isActiveRef.current) return
      scheduleFitDuringResize()
      schedulePtyResizeCommit()
    })
    resizeObserverRef.current = resizeObserver

    if (isActiveRef.current) {
      resizeObserver.observe(container)
    }

    return () => {
      resizeObserver.disconnect()
      resizeObserverRef.current = null

      if (resizeRafRef.current !== null) {
        window.cancelAnimationFrame(resizeRafRef.current)
        resizeRafRef.current = null
      }

      if (resizeEndTimerRef.current !== null) {
        window.clearTimeout(resizeEndTimerRef.current)
        resizeEndTimerRef.current = null
      }

      if (activateFitTimerRef.current !== null) {
        window.clearTimeout(activateFitTimerRef.current)
        activateFitTimerRef.current = null
      }

      if (outputFlushRafRef.current !== null) {
        window.cancelAnimationFrame(outputFlushRafRef.current)
        outputFlushRafRef.current = null
      }
      pendingOutputRef.current = []
      pendingOutputBytesRef.current = 0

      webglAddonRef.current?.dispose()
      webglAddonRef.current = null

      unlistenOutput?.()
      unlistenExit?.()
      invoke("kill_pty", { tabId }).catch(() => {})
      term.dispose()
    }
  }, [
    tabId,
    onPidChange,
    fitTerminalOnly,
    resolveTerminalBackground,
    enqueueOutput,
    flushPendingOutput,
    scheduleFitDuringResize,
    schedulePtyResizeCommit,
  ])

  useEffect(() => {
    const container = containerRef.current
    const resizeObserver = resizeObserverRef.current
    if (!container || !resizeObserver) return

    if (isActive) {
      resizeObserver.observe(container)
      activateFitTimerRef.current = window.setTimeout(() => {
        activateFitTimerRef.current = null
        fitAndSyncPty()
      }, TAB_ACTIVATE_REFIT_DELAY_MS)
      return
    }

    resizeObserver.unobserve(container)

    if (resizeRafRef.current !== null) {
      window.cancelAnimationFrame(resizeRafRef.current)
      resizeRafRef.current = null
    }

    if (resizeEndTimerRef.current !== null) {
      window.clearTimeout(resizeEndTimerRef.current)
      resizeEndTimerRef.current = null
    }

    if (activateFitTimerRef.current !== null) {
      window.clearTimeout(activateFitTimerRef.current)
      activateFitTimerRef.current = null
    }
  }, [isActive, fitAndSyncPty])

  return (
    <div
      ref={containerRef}
      style={{
        width: "100%",
        height: "100%",
        overflow: "hidden",
        backgroundColor: "hsl(var(--background))",
      }}
    />
  )
}
