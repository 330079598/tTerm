import "@/components/TerminalTab.css"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { Terminal } from "@xterm/xterm"
import { FitAddon } from "@xterm/addon-fit"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { Globe, Pin, PinOff, PlugZap, RefreshCcw } from "lucide-react"
import { SftpDrawer } from "@/components/SftpDrawer"
import { useConfig } from "@/contexts/ConfigContext"
import { useTranslation } from "react-i18next"
import type { Tab } from "@/types/tab"
import "@xterm/xterm/css/xterm.css"

interface TerminalTabProps {
  tabId: string
  sessionNonce?: number
  isActive: boolean
  connectionHeaderPinned?: boolean
  connection?: Tab["connection"]
  onPidChange?: (pid: number) => void
  onReconnectRequest?: () => void
  onPinConnectionHeader?: () => void
  onUnpinConnectionHeader?: () => void
}

type ConnectionState = "connecting" | "connected" | "reconnecting" | "disconnected" | "error"

type HostKeyPromptState = {
  requestId: string
  profileName: string
  host: string
  port: number
  algorithm: string
  fingerprint: string
  reason: string
  knownFingerprint?: string
}

const FALLBACK_TERMINAL_BACKGROUND = "#111827"
const TAB_ACTIVATE_REFIT_DELAY_MS = 32
const STATUS_RECONNECT_PREFIX = "[SSH disconnected. Reconnect attempt"
const STATUS_RECONNECTED = "[SSH reconnected]"
const STATUS_CONNECTING = "[Connecting"

function getConnectionDisplay(connection?: TerminalTabProps["connection"]): string {
  if (!connection || connection.type === "terminal") {
    return "Local shell"
  }

  const host = connection.host || "unknown-host"
  const port = connection.port ?? 22
  const address = `${host}:${port}`
  return connection.username ? `${connection.username}@${address}` : address
}

function getConnectionStateLabel(
  state: ConnectionState,
  t: (key: string, options?: Record<string, unknown>) => string
): string {
  switch (state) {
    case "connecting":
      return t("sessionHeader.connecting", { defaultValue: "Connecting" })
    case "connected":
      return t("sessionHeader.connected", { defaultValue: "Connected" })
    case "reconnecting":
      return t("sessionHeader.reconnecting", { defaultValue: "Reconnecting" })
    case "disconnected":
      return t("sessionHeader.disconnected", { defaultValue: "Disconnected" })
    case "error":
      return t("sessionHeader.error", { defaultValue: "Error" })
  }
}

export const TerminalTab: React.FC<TerminalTabProps> = ({
  tabId,
  sessionNonce = 0,
  isActive,
  connectionHeaderPinned = true,
  connection,
  onPidChange,
  onReconnectRequest,
  onPinConnectionHeader,
  onUnpinConnectionHeader,
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
  const sessionResetKey = `${tabId}:${sessionNonce}:${connection?.type ?? "terminal"}`
  const defaultConnectionState: ConnectionState =
    connection?.type === "ssh" ? "connecting" : "connected"

  const [hostKeyPromptState, setHostKeyPromptState] = useState<{
    sessionKey: string
    value: HostKeyPromptState
  } | null>(null)
  const [exitErrorState, setExitErrorState] = useState<{
    sessionKey: string
    value: string
  } | null>(null)
  const [connectionStateState, setConnectionStateState] = useState<{
    sessionKey: string
    value: ConnectionState
  } | null>(null)
  const [showSftpDrawer, setShowSftpDrawer] = useState(false)

  const hostKeyPrompt =
    hostKeyPromptState?.sessionKey === sessionResetKey ? hostKeyPromptState.value : null
  const exitError = exitErrorState?.sessionKey === sessionResetKey ? exitErrorState.value : null
  const connectionState =
    connectionStateState?.sessionKey === sessionResetKey
      ? connectionStateState.value
      : defaultConnectionState

  const setHostKeyPrompt = useCallback(
    (value: HostKeyPromptState | null) => {
      setHostKeyPromptState(value ? { sessionKey: sessionResetKey, value } : null)
    },
    [sessionResetKey]
  )

  const setExitError = useCallback(
    (value: string | null) => {
      setExitErrorState(value ? { sessionKey: sessionResetKey, value } : null)
    },
    [sessionResetKey]
  )

  const setConnectionState = useCallback(
    (value: ConnectionState) => {
      setConnectionStateState({ sessionKey: sessionResetKey, value })
    },
    [sessionResetKey]
  )

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
    let disposed = false

    // Register event listeners before spawning the PTY so no output is lost
    Promise.all([
      listen<string>(`pty-output-${tabId}`, (event) => {
        const payload = event.payload
        if (payload.includes(STATUS_RECONNECT_PREFIX)) {
          setConnectionState("reconnecting")
        } else if (payload.includes(STATUS_RECONNECTED)) {
          setConnectionState("connected")
          setExitError(null)
        } else if (payload.includes(STATUS_CONNECTING)) {
          setConnectionState("connecting")
        } else if (connectionRef.current?.type === "ssh" && payload.trim().length > 0) {
          setConnectionState("connected")
          setExitError(null)
        }
        term.write(payload)
      }),
      listen(`pty-exit-${tabId}`, (event) => {
        term.writeln("\r\n\x1b[33m[Process exited]\x1b[0m")
        const reason = event.payload as string | null | undefined
        if (connectionRef.current?.type === "ssh") {
          if (reason) {
            setExitError(reason)
            setConnectionState("error")
          } else {
            setConnectionState("disconnected")
          }
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
        setConnectionState("connecting")
      }),
    ])
      .then(([unOut, unExit, unHostPrompt]) => {
        unlistenOutput = unOut
        unlistenExit = unExit
        unlistenHostPrompt = unHostPrompt

        if (disposed) {
          unlistenOutput?.()
          unlistenExit?.()
          unlistenHostPrompt?.()
          return null
        }

        setConnectionState(connectionRef.current?.type === "ssh" ? "connecting" : "connected")

        return invoke<number>("create_pty", {
          tabId,
          rows: term.rows,
          cols: term.cols,
          connection: connectionRef.current,
        })
      })
      .then((pid) => {
        if (pid == null) return

        if (disposed) {
          invoke("kill_pty", { tabId }).catch(() => {})
          return
        }

        if (connectionRef.current?.type !== "ssh") {
          setConnectionState("connected")
        }
        onPidChange?.(pid)
      })
      .catch((e) => {
        if (disposed) return
        if (connectionRef.current?.type === "ssh") {
          setConnectionState("error")
          setExitError(String(e))
        }
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
      disposed = true

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
      termRef.current = null
      fitAddonRef.current = null
      initializedRef.current = false
      lastPtySizeRef.current = null
    }
  }, [
    fitTerminalOnly,
    onPidChange,
    resolveTerminalBackground,
    scheduleFitDuringResize,
    setConnectionState,
    setExitError,
    setHostKeyPrompt,
    sessionNonce,
    tabId,
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

  const handleReconnect = async () => {
    setExitError(null)
    setConnectionState("connecting")
    onReconnectRequest?.()
  }

  const handleToggleSftpDrawer = useCallback(() => {
    setShowSftpDrawer((current) => !current)
  }, [])

  const showConnectionHeader = connection?.type === "ssh" && connectionHeaderPinned
  const showPinnedToggle = connection?.type === "ssh" && !connectionHeaderPinned

  return (
    <div className="terminal-tab-shell">
      {showPinnedToggle && (
        <button
          type="button"
          className="connection-header-restore"
          onClick={onPinConnectionHeader}
          title={t("sessionHeader.pin", { defaultValue: "Pin" })}
        >
          <Pin size={14} />
          <span>{t("sessionHeader.connectionInfo", { defaultValue: "Connection" })}</span>
        </button>
      )}

      {showConnectionHeader && (
        <div className="connection-header">
          <div className="connection-header-main">
            <span
              className={`connection-status-pill is-${connectionState}`}
              title={getConnectionStateLabel(connectionState, t)}
              aria-label={getConnectionStateLabel(connectionState, t)}
            >
              <span className="connection-status-dot" />
              <span className="sr-only">{getConnectionStateLabel(connectionState, t)}</span>
            </span>
            <div className="connection-meta">
              <div className="connection-primary">{getConnectionDisplay(connection)}</div>
            </div>
          </div>

          <div className="connection-header-actions">
            <button
              type="button"
              className="connection-action"
              onClick={handleReconnect}
              title={t("sessionHeader.reconnect", { defaultValue: "Reconnect" })}
            >
              <RefreshCcw size={14} />
              <span>{t("sessionHeader.reconnect", { defaultValue: "Reconnect" })}</span>
            </button>
            <button
              type="button"
              className="connection-action"
              onClick={handleToggleSftpDrawer}
              title={t("sessionHeader.sftp", { defaultValue: "SFTP" })}
            >
              <Globe size={14} />
              <span>{t("sessionHeader.sftp", { defaultValue: "SFTP" })}</span>
            </button>
            <button
              type="button"
              className="connection-action"
              onClick={onUnpinConnectionHeader}
              title={t("sessionHeader.unpin", { defaultValue: "Unpin" })}
            >
              <PinOff size={14} />
              <span>{t("sessionHeader.unpin", { defaultValue: "Unpin" })}</span>
            </button>
          </div>
        </div>
      )}

      <div className="terminal-surface">
        <SftpDrawer
          tabId={tabId}
          visible={showSftpDrawer}
          connection={connection}
          onClose={() => setShowSftpDrawer(false)}
        />

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
          <div className="host-key-dialog-overlay">
            <div className="host-key-dialog-content">
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
                    handleReconnect().catch(() => {})
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
                <PlugZap size={14} />
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
