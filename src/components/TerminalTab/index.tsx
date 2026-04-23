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
  getConnectionDisplay,
  STATUS_CONNECTING,
  TAB_ACTIVATE_REFIT_DELAY_MS,
} from "@/components/TerminalTab/terminalTabUtils"
import type {
  ConnectionState,
  HostKeyPromptState,
  TerminalTabProps,
} from "@/components/TerminalTab/types"
import { toast } from "@/hooks/use-toast"
import { useConfig } from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"

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
  const onPidChangeRef = useRef(onPidChange)
  const onReconnectRequestRef = useRef(onReconnectRequest)
  const { config } = useConfig()
  const { currentTheme, getTheme } = useTheme()
  const { t } = useTranslation()
  const initialFontFamily = useRef(config.font_family)
  const initialFontSize = useRef(config.font_size)
  const initialCursorStyle = useRef(config.cursor_style)
  const initialScrollbackLines = useRef(config.scrollback_lines)
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
    onPidChangeRef.current = onPidChange
  }, [onPidChange])

  useEffect(() => {
    onReconnectRequestRef.current = onReconnectRequest
  }, [onReconnectRequest])

  const resolveTerminalTheme = useCallback(() => {
    return { ...(getTheme(currentTheme)?.terminal ?? getTheme("default")!.terminal) }
  }, [currentTheme, getTheme])

  // Keep the first palette for terminal creation; later theme changes update xterm in place.
  const initialTerminalThemeRef =
    useRef<ReturnType<typeof resolveTerminalTheme>>(resolveTerminalTheme())

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
    term.options.cursorStyle = config.cursor_style
    if (isActiveRef.current) {
      fitAndSyncPty()
    }
  }, [config.cursor_style, config.font_family, config.font_size, fitAndSyncPty])

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

    term.options.theme = resolveTerminalTheme()
  }, [currentTheme, resolveTerminalTheme])

  useEffect(() => {
    const container = containerRef.current
    if (!container || initializedRef.current) return
    initializedRef.current = true
    waitingForReconnectRef.current = false
    passwordPromptActiveRef.current = false

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: initialCursorStyle.current,
      scrollback: initialScrollbackLines.current === 0 ? 10000000 : initialScrollbackLines.current,
      fontSize: initialFontSize.current,
      fontFamily: initialFontFamily.current,
      fontWeight: "normal",
      fontWeightBold: "bold",
      letterSpacing: 0,
      lineHeight: 1.0,
      theme: initialTerminalThemeRef.current,
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
          const promptUsername = match[1].trim() // Match the sudo prompt username before offering password paste
          const savedUsername = connectionRef.current?.username
          const profileId = connectionRef.current?.profileId
          const profileName = connectionRef.current?.profileName

          // Only show prompt if username matches
          if (savedUsername && promptUsername === savedUsername && profileName) {
            // Try to get password from backend
            invoke<string | null>("get_saved_password_for_sudo", {
              profileId,
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
        onPidChangeRef.current?.(pid)
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
      waitingForReconnectRef.current = false
      passwordPromptActiveRef.current = false
    }
  }, [
    fitTerminalOnly,
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

  const handleConnectionHeaderMouseDown = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (!showSftpDrawer) {
        return
      }

      const target = event.target as HTMLElement | null
      if (target?.closest("button")) {
        return
      }

      setShowSftpDrawer(false)
    },
    [showSftpDrawer]
  )

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
        onBackgroundMouseDown={handleConnectionHeaderMouseDown}
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
