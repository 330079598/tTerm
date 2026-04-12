import "@/components/TerminalTab.css"
import "@xterm/xterm/css/xterm.css"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { FitAddon } from "@xterm/addon-fit"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { Terminal } from "@xterm/xterm"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useTranslation } from "react-i18next"

import { SftpDrawer } from "@/components/SftpDrawer"
import { ConnectionHeader } from "@/components/TerminalTab/ConnectionHeader"
import { HostKeyPromptDialog } from "@/components/TerminalTab/HostKeyPromptDialog"
import {
  FALLBACK_TERMINAL_BACKGROUND,
  STATUS_CONNECTING,
  TAB_ACTIVATE_REFIT_DELAY_MS,
  getConnectionDisplay,
} from "@/components/TerminalTab/terminalTabUtils"
import type {
  ConnectionState,
  HostKeyPromptState,
  TerminalTabProps,
} from "@/components/TerminalTab/types"
import { toast } from "@/hooks/use-toast"
import { useConfig } from "@/contexts/ConfigContext"

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
  const waitingForReconnectRef = useRef(false)
  const onReconnectRequestRef = useRef(onReconnectRequest)
  const { config } = useConfig()
  const { t } = useTranslation()
  const initialFontFamily = useRef(config.font_family)
  const initialFontSize = useRef(config.font_size)
  const sessionResetKey = `${tabId}:${sessionNonce}:${connection?.type ?? "terminal"}`
  const defaultConnectionState: ConnectionState =
    connection?.type === "ssh" ? "connecting" : "connected"
  const passwordPromptActiveRef = useRef(false)

  const [hostKeyPromptState, setHostKeyPromptState] = useState<{
    sessionKey: string
    value: HostKeyPromptState
  } | null>(null)
  const [connectionStateState, setConnectionStateState] = useState<{
    sessionKey: string
    value: ConnectionState
  } | null>(null)
  const [showSftpDrawer, setShowSftpDrawer] = useState(false)

  const hostKeyPrompt =
    hostKeyPromptState?.sessionKey === sessionResetKey ? hostKeyPromptState.value : null
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

  useEffect(() => {
    onReconnectRequestRef.current = onReconnectRequest
  }, [onReconnectRequest])

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

  useEffect(() => {
    const term = termRef.current
    if (!term) return

    term.options.fontFamily = config.font_family
    term.options.fontSize = config.font_size
    if (isActiveRef.current) {
      fitAndSyncPty()
    }
  }, [config.font_family, config.font_size, fitAndSyncPty])

  useEffect(() => {
    const term = termRef.current
    if (!term) return

    // 0 means unlimited scrollback in our app, but xterm.js uses a very large number
    // xterm.js doesn't support true unlimited, so we use a very large number (10 million)
    term.options.scrollback = config.scrollback_lines === 0 ? 10000000 : config.scrollback_lines
  }, [config.scrollback_lines])

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
      scrollback: config.scrollback_lines === 0 ? 10000000 : config.scrollback_lines,
      fontSize: initialFontSize.current,
      fontFamily: initialFontFamily.current,
      fontWeight: "normal",
      fontWeightBold: "bold",
      letterSpacing: 0,
      lineHeight: 1.0,
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

    term.open(container)
    term.focus()
    fitTerminalOnly()

    term.onData((data) => {
      if (waitingForReconnectRef.current) {
        waitingForReconnectRef.current = false
        setConnectionState("connecting")
        onReconnectRequestRef.current?.()
        return
      }

      // Check if password prompt is active
      if (passwordPromptActiveRef.current) {
        // Clear the hint text by moving cursor back and clearing to end of line
        term.write("\r\x1b[K")

        // Check if Enter key is pressed
        if (data === "\r") {
          const savedPassword = connectionRef.current?.password
          if (savedPassword) {
            // Send the saved password followed by Enter
            invoke("write_pty", { tabId, data: savedPassword + "\n" }).catch(() => {})
            passwordPromptActiveRef.current = false
            return
          }
        }

        // For any other key, disable password prompt and pass through the key
        passwordPromptActiveRef.current = false
      }

      invoke("write_pty", { tabId, data }).catch(() => {})
    })

    let unlistenOutput: (() => void) | null = null
    let unlistenExit: (() => void) | null = null
    let unlistenHostPrompt: (() => void) | null = null
    let disposed = false

    Promise.all([
      listen<string>(`pty-output-${tabId}`, (event) => {
        const payload = event.payload
        if (payload.includes(STATUS_CONNECTING)) {
          setConnectionState("connecting")
        } else if (connectionRef.current?.type === "ssh" && payload.trim().length > 0) {
          setConnectionState("connected")
        }

        // Detect sudo password prompt
        const sudoPasswordPattern = /^\[sudo\] password for ([^:]+):\s*$/im
        const match = payload.match(sudoPasswordPattern)

        if (match && !passwordPromptActiveRef.current) {
          const promptUsername = match[1].trim() // 去除首尾空格
          const savedUsername = connectionRef.current?.username
          const profileName = connectionRef.current?.profileName

          // Only show prompt if username matches
          if (savedUsername && promptUsername === savedUsername && profileName) {
            // Try to get password from backend
            invoke<string | null>("get_saved_password_for_sudo", {
              profileName,
            })
              .then((password) => {
                if (password) {
                  passwordPromptActiveRef.current = true
                  // Store password temporarily for this prompt
                  connectionRef.current = {
                    ...connectionRef.current,
                    password,
                  }
                  // Write hint text to terminal with formatted style
                  // Format: [TTerm in black background] [Gray text message]
                  const pasteHint =
                    "\x1b[100m\x1b[36m tTerm \x1b[0m " + // Black text on bright black background
                    "\x1b[90mPress Enter to paste saved password\x1b[0m" // Gray text
                  term.write(pasteHint)
                }
              })
              .catch((err) => {
                console.error("Failed to get saved password:", err)
              })
          }
        }

        term.write(payload)
      }),
      listen(`pty-exit-${tabId}`, (event) => {
        const reason = event.payload as string | null | undefined
        if (connectionRef.current?.type === "ssh") {
          const displayAddress = getConnectionDisplay(connectionRef.current)
          term.writeln(`\r\n\x1b[33m${displayAddress}: session closed\x1b[0m`)
          term.writeln("\x1b[36mPress any key to reconnect\x1b[0m")

          if (reason) {
            setConnectionState("error")
          } else {
            setConnectionState("disconnected")
          }
          waitingForReconnectRef.current = true
        } else {
          term.writeln("\r\n\x1b[33m[Process exited]\x1b[0m")
        }
      }),
      listen<HostKeyPromptState>(`ssh-hostkey-prompt-${tabId}`, async (event) => {
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
      .catch((error) => {
        if (disposed) return
        if (connectionRef.current?.type === "ssh") {
          setConnectionState("error")
        }
        term.writeln(`\x1b[31mFailed to start terminal: ${error}\x1b[0m`)
      })

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
    config.scrollback_lines,
    fitTerminalOnly,
    onPidChange,
    resolveTerminalBackground,
    scheduleFitDuringResize,
    setConnectionState,
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

  const handleReconnect = useCallback(() => {
    waitingForReconnectRef.current = false
    setConnectionState("connecting")
    onReconnectRequest?.()
  }, [onReconnectRequest, setConnectionState])

  const handleToggleSftpDrawer = useCallback(() => {
    setShowSftpDrawer((current) => !current)
  }, [])

  const handleTerminalContextMenu = useCallback(
    async (event: React.MouseEvent<HTMLDivElement>) => {
      event.preventDefault()

      const term = termRef.current
      term?.focus()

      if (showSftpDrawer) {
        setShowSftpDrawer(false)
      }

      const selection = term?.hasSelection() ? term.getSelection() : ""

      if (selection) {
        try {
          await invoke("plugin:clipboard-manager|write_text", { text: selection })
          term?.clearSelection()
        } catch (error) {
          console.error("Failed to copy terminal selection:", error)
          toast({
            title: t("terminalContext.copyFailedTitle", {
              defaultValue: "Copy failed",
            }),
            description: t("terminalContext.copyFailedDescription", {
              defaultValue: "Unable to copy the current terminal selection.",
            }),
            variant: "destructive",
          })
        }

        return
      }

      try {
        const clipboardText = await invoke<string>("plugin:clipboard-manager|read_text")
        if (!clipboardText) {
          return
        }

        await invoke("write_pty", { tabId, data: clipboardText })
      } catch (error) {
        console.error("Failed to paste into terminal:", error)
        toast({
          title: t("terminalContext.pasteFailedTitle", {
            defaultValue: "Paste failed",
          }),
          description: t("terminalContext.pasteFailedDescription", {
            defaultValue: "Unable to paste clipboard contents into the terminal.",
          }),
          variant: "destructive",
        })
      }
    },
    [showSftpDrawer, t, tabId]
  )

  return (
    <div className="terminal-tab-shell">
      <ConnectionHeader
        connection={connection}
        connectionHeaderPinned={connectionHeaderPinned}
        connectionState={connectionState}
        onPinConnectionHeader={onPinConnectionHeader}
        onReconnect={handleReconnect}
        onToggleSftpDrawer={handleToggleSftpDrawer}
        onUnpinConnectionHeader={onUnpinConnectionHeader}
      />

      <div className="terminal-surface">
        <SftpDrawer
          tabId={tabId}
          visible={showSftpDrawer}
          connection={connection}
          onClose={() => setShowSftpDrawer(false)}
        />

        <div
          ref={containerRef}
          onMouseDown={() => {
            termRef.current?.focus()
            if (showSftpDrawer) {
              setShowSftpDrawer(false)
            }
          }}
          onContextMenu={handleTerminalContextMenu}
          style={{
            width: "100%",
            height: "100%",
            overflow: "hidden",
            backgroundColor: "hsl(var(--background))",
          }}
        />

        <HostKeyPromptDialog hostKeyPrompt={hostKeyPrompt} setHostKeyPrompt={setHostKeyPrompt} />
      </div>
    </div>
  )
}
