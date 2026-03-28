import "@/components/TerminalTab.css"
import React, { useEffect, useRef, useCallback, useState } from "react"
import { Terminal } from "@xterm/xterm"
import { FitAddon } from "@xterm/addon-fit"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useConfig } from "@/contexts/ConfigContext"
import { useTranslation } from "react-i18next"
import "@xterm/xterm/css/xterm.css"

interface TerminalTabProps {
  tabId: string
  isActive: boolean
  connection?: {
    type?: "terminal" | "ssh"
    profileName?: string
    host?: string
    port?: number
    username?: string
    password?: string
    rememberPassword?: boolean
    reconnect?: boolean
    reconnectDelaySecs?: number
    reconnectMaxDelaySecs?: number
    reconnectMaxRetries?: number
    keepaliveIntervalSecs?: number
    keepaliveCountMax?: number
    privateKeyPath?: string
    privateKeyPassphrase?: string
  }
  onPidChange?: (pid: number) => void
  onReconnectRequest?: () => void
}

const FALLBACK_TERMINAL_BACKGROUND = "#111827"
const TAB_ACTIVATE_REFIT_DELAY_MS = 32

export const TerminalTab: React.FC<TerminalTabProps> = ({
  tabId,
  isActive,
  connection,
  onPidChange,
  onReconnectRequest,
}) => {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const resizeObserverRef = useRef<ResizeObserver | null>(null)
  const resizeRafRef = useRef<number | null>(null)
  const resizeEndTimerRef = useRef<number | null>(null)
  const activateFitTimerRef = useRef<number | null>(null)
  const lastPtySizeRef = useRef<{ rows: number; cols: number } | null>(null)
  const connectionRef = useRef(connection)
  const isActiveRef = useRef(isActive)
  const initializedRef = useRef(false)
  const { config } = useConfig()
  const { t } = useTranslation()
  const initialFontFamily = useRef(config.font_family)
  const initialFontSize = useRef(config.font_size)

  const [hostKeyPrompt, setHostKeyPrompt] = useState<{
    requestId: string
    profileName: string
    host: string
    port: number
    algorithm: string
    fingerprint: string
    reason: string
    knownFingerprint?: string
  } | null>(null)

  const [exitError, setExitError] = useState<string | null>(null)

  useEffect(() => {
    isActiveRef.current = isActive
  }, [isActive])

  useEffect(() => {
    connectionRef.current = connection
  }, [connection])

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
    if (resizeRafRef.current !== null) {
      window.cancelAnimationFrame(resizeRafRef.current)
    }

    resizeRafRef.current = window.requestAnimationFrame(() => {
      resizeRafRef.current = null
      if (!isActiveRef.current) return
      fitAndSyncPty()
    })
  }, [fitAndSyncPty])

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

    termRef.current = term
    fitAddonRef.current = fitAddon

    // Mount xterm into the container and perform initial fit
    term.open(container)
    term.focus()
    fitTerminalOnly()

    // Forward user keystrokes to the backend PTY
    term.onData((data) => {
      invoke("write_pty", { tabId, data }).catch(() => {})
    })

    let unlistenOutput: (() => void) | null = null
    let unlistenExit: (() => void) | null = null
    let unlistenHostPrompt: (() => void) | null = null

    // Register event listeners before spawning the PTY so no output is lost
    Promise.all([
      listen<string>(`pty-output-${tabId}`, (event) => {
        term.write(event.payload)
      }),
      listen(`pty-exit-${tabId}`, (event) => {
        term.writeln("\r\n\x1b[33m[Process exited]\x1b[0m")
        const reason = event.payload as string | null | undefined
        if (reason && connectionRef.current?.type === "ssh") {
          setExitError(reason)
        }
      }),
      listen<{
        requestId: string
        profileName: string
        host: string
        port: number
        algorithm: string
        fingerprint: string
        reason: string
        knownFingerprint?: string
      }>(`ssh-hostkey-prompt-${tabId}`, async (event) => {
        setHostKeyPrompt(event.payload)
      }),
    ])
      .then(([unOut, unExit, unHostPrompt]) => {
        unlistenOutput = unOut
        unlistenExit = unExit
        unlistenHostPrompt = unHostPrompt
        return invoke<number>("create_pty", {
          tabId,
          rows: term.rows,
          cols: term.cols,
          connection: connectionRef.current,
        })
      })
      .then((pid) => {
        onPidChange?.(pid)
      })
      .catch((e) => {
        term.writeln(`\x1b[31mFailed to start terminal: ${e}\x1b[0m`)
      })

    // During live-resize we fit+sync in RAF (cancelled and rescheduled each callback
    // so xterm always reflects the latest container size); a trailing debounce ensures
    // one final sync after the drag settles.
    const resizeObserver = new ResizeObserver(() => {
      if (!isActiveRef.current) return
      scheduleFitDuringResize()
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

      unlistenOutput?.()
      unlistenExit?.()
      unlistenHostPrompt?.()
      invoke("kill_pty", { tabId }).catch(() => {})
      term.dispose()
    }
  }, [tabId, onPidChange, fitTerminalOnly, resolveTerminalBackground, scheduleFitDuringResize])

  useEffect(() => {
    const container = containerRef.current
    const resizeObserver = resizeObserverRef.current
    if (!container || !resizeObserver) return

    if (isActive) {
      resizeObserver.observe(container)
      activateFitTimerRef.current = window.setTimeout(() => {
        activateFitTimerRef.current = null
        fitAndSyncPty()
        termRef.current?.focus()
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
    <div style={{ position: "relative", width: "100%", height: "100%" }}>
      <div
        ref={containerRef}
        onMouseDown={() => termRef.current?.focus()}
        style={{
          width: "100%",
          height: "100%",
          overflow: "hidden",
          backgroundColor: "hsl(var(--background))",
        }}
      />

      {/* Host Key Verification Dialog */}
      {hostKeyPrompt && (
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            backgroundColor: "rgba(0,0,0,0.6)",
            zIndex: 10,
          }}
        >
          <div
            style={{
              background: "hsl(var(--card))",
              border: "1px solid hsl(var(--border))",
              borderRadius: 8,
              padding: "24px",
              maxWidth: 480,
              width: "90%",
              color: "hsl(var(--foreground))",
            }}
          >
            <h3 style={{ margin: "0 0 12px", fontSize: 16, fontWeight: 600 }}>
              {hostKeyPrompt.reason === "mismatch"
                ? t("ssh.hostKeyMismatch")
                : t("ssh.unknownHostKey")}
            </h3>
            <div
              style={{
                fontSize: 13,
                lineHeight: 1.6,
                marginBottom: 16,
                fontFamily: "monospace",
                background: "hsl(var(--muted))",
                padding: "10px 12px",
                borderRadius: 4,
              }}
            >
              <div>
                <b>{t("ssh.host")}:</b> {hostKeyPrompt.host}:{hostKeyPrompt.port}
              </div>
              <div>
                <b>{t("ssh.algorithm")}:</b> {hostKeyPrompt.algorithm}
              </div>
              <div>
                <b>{t("ssh.fingerprint")}:</b> {hostKeyPrompt.fingerprint}
              </div>
              {hostKeyPrompt.knownFingerprint && (
                <div style={{ color: "hsl(var(--destructive))" }}>
                  <b>{t("ssh.knownFingerprint")}:</b> {hostKeyPrompt.knownFingerprint}
                </div>
              )}
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button
                onClick={async () => {
                  await invoke("respond_ssh_host_key_prompt", {
                    requestId: hostKeyPrompt.requestId,
                    trust: false,
                  }).catch(() => {})
                  setHostKeyPrompt(null)
                }}
                style={{
                  padding: "6px 16px",
                  borderRadius: 4,
                  border: "1px solid hsl(var(--border))",
                  background: "hsl(var(--muted))",
                  color: "hsl(var(--foreground))",
                  cursor: "pointer",
                }}
              >
                {t("ssh.reject")}
              </button>
              <button
                onClick={async () => {
                  await invoke("respond_ssh_host_key_prompt", {
                    requestId: hostKeyPrompt.requestId,
                    trust: true,
                  }).catch(() => {})
                  setHostKeyPrompt(null)
                }}
                style={{
                  padding: "6px 16px",
                  borderRadius: 4,
                  border: "none",
                  background: "hsl(var(--primary))",
                  color: "hsl(var(--primary-foreground))",
                  cursor: "pointer",
                }}
              >
                {t("ssh.trust")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Exit Error Banner */}
      {exitError && (
        <div
          style={{
            position: "absolute",
            bottom: 0,
            left: 0,
            right: 0,
            background: "hsl(var(--destructive))",
            color: "hsl(var(--destructive-foreground))",
            padding: "10px 16px",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            zIndex: 10,
            fontSize: 13,
          }}
        >
          <span>{exitError}</span>
          <div style={{ display: "flex", gap: 8 }}>
            {onReconnectRequest && (
              <button
                onClick={() => {
                  setExitError(null)
                  onReconnectRequest()
                }}
                style={{
                  padding: "4px 12px",
                  borderRadius: 4,
                  border: "none",
                  background: "rgba(255,255,255,0.2)",
                  color: "inherit",
                  cursor: "pointer",
                  fontSize: 13,
                }}
              >
                {t("ssh.retry")}
              </button>
            )}
            <button
              onClick={() => setExitError(null)}
              style={{
                padding: "4px 12px",
                borderRadius: 4,
                border: "none",
                background: "transparent",
                color: "inherit",
                cursor: "pointer",
                fontSize: 18,
                lineHeight: 1,
              }}
            >
              ×
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
