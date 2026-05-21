import { useEffect } from "react"
import { FitAddon } from "@xterm/addon-fit"
import { SearchAddon, type ISearchResultChangeEvent } from "@xterm/addon-search"
import { Unicode11Addon } from "@xterm/addon-unicode11"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { WebglAddon } from "@xterm/addon-webgl"
import { type IDisposable, Terminal } from "@xterm/xterm"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { openUrl } from "@tauri-apps/plugin-opener"
import { platform } from "@tauri-apps/plugin-os"

import { getConnectionDisplay, STATUS_CONNECTING } from "@/components/TerminalTab/terminalTabUtils"
import type {
  ConnectionState,
  HostKeyPromptState,
  SshConnectionProgress,
  TerminalTabProps,
} from "@/components/TerminalTab/types"

type UseTerminalLifecycleOptions = {
  activateFitTimerRef: React.MutableRefObject<number | null>
  connectionRef: React.MutableRefObject<TerminalTabProps["connection"]>
  containerRef: React.RefObject<HTMLDivElement>
  fitAddonRef: React.MutableRefObject<FitAddon | null>
  fitTerminalOnly: () => void
  initializedRef: React.MutableRefObject<boolean>
  initialCursorStyle: React.MutableRefObject<Terminal["options"]["cursorStyle"]>
  initialFontFamily: React.MutableRefObject<string>
  initialFontSize: React.MutableRefObject<number>
  initialScrollbackLines: React.MutableRefObject<number>
  initialTerminalThemeRef: React.MutableRefObject<NonNullable<Terminal["options"]["theme"]>>
  isActiveRef: React.MutableRefObject<boolean>
  lastPtySizeRef: React.MutableRefObject<{ rows: number; cols: number } | null>
  onPidChangeRef: React.MutableRefObject<TerminalTabProps["onPidChange"]>
  onReconnectRequestRef: React.MutableRefObject<TerminalTabProps["onReconnectRequest"]>
  passwordPromptActiveRef: React.MutableRefObject<boolean>
  resizeObserverRef: React.MutableRefObject<ResizeObserver | null>
  resizeRafRef: React.MutableRefObject<number | null>
  scheduleFitDuringResize: () => void
  searchAddonRef: React.MutableRefObject<SearchAddon | null>
  searchResultsDisposableRef: React.MutableRefObject<IDisposable | null>
  setConnectionState: (value: ConnectionState) => void
  setHostKeyPrompt: (value: HostKeyPromptState | null) => void
  setConnectionProgress: (value: SshConnectionProgress | null) => void
  setSearchResults: React.Dispatch<React.SetStateAction<ISearchResultChangeEvent>>
  sessionNonce: number
  tabId: string
  termRef: React.MutableRefObject<Terminal | null>
  waitingForReconnectRef: React.MutableRefObject<boolean>
}

const LINK_MODIFIER_IS_CMD = (() => {
  try {
    return platform() === "macos"
  } catch {
    if (typeof navigator !== "undefined") {
      const platformHint = `${navigator.platform} ${navigator.userAgent}`.toLowerCase()
      if (platformHint.includes("mac")) {
        return true
      }
    }

    return false
  }
})()

function isLinkOpenModifierPressed(event: MouseEvent) {
  return LINK_MODIFIER_IS_CMD ? event.metaKey : event.ctrlKey
}

export function useTerminalLifecycle({
  activateFitTimerRef,
  connectionRef,
  containerRef,
  fitAddonRef,
  fitTerminalOnly,
  initializedRef,
  initialCursorStyle,
  initialFontFamily,
  initialFontSize,
  initialScrollbackLines,
  initialTerminalThemeRef,
  isActiveRef,
  lastPtySizeRef,
  onPidChangeRef,
  onReconnectRequestRef,
  passwordPromptActiveRef,
  resizeObserverRef,
  resizeRafRef,
  scheduleFitDuringResize,
  searchAddonRef,
  searchResultsDisposableRef,
  setConnectionState,
  setHostKeyPrompt,
  setConnectionProgress,
  setSearchResults,
  sessionNonce,
  tabId,
  termRef,
  waitingForReconnectRef,
}: UseTerminalLifecycleOptions) {
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
      allowProposedApi: true,
    })

    const fitAddon = new FitAddon()
    const searchAddon = new SearchAddon({ highlightLimit: 2000 })
    term.loadAddon(fitAddon)
    term.loadAddon(searchAddon)
    term.loadAddon(
      new WebLinksAddon((event, uri) => {
        if (!isLinkOpenModifierPressed(event)) {
          return
        }

        event.preventDefault()
        void openUrl(uri).catch((error) => {
          console.error("Failed to open terminal link:", error)
        })
      })
    )
    term.loadAddon(new Unicode11Addon())
    term.unicode.activeVersion = "11"

    try {
      term.loadAddon(new WebglAddon())
    } catch {
      // WebGL not supported in this environment; fall back to canvas renderer
    }

    termRef.current = term
    fitAddonRef.current = fitAddon
    searchAddonRef.current = searchAddon
    searchResultsDisposableRef.current = searchAddon.onDidChangeResults((results) => {
      setSearchResults(results)
    })

    term.open(container)
    if (isActiveRef.current) {
      term.focus()
    }
    fitTerminalOnly()

    term.onData((data) => {
      if (waitingForReconnectRef.current) {
        waitingForReconnectRef.current = false
        setConnectionState("connecting")
        onReconnectRequestRef.current?.()
        return
      }

      if (passwordPromptActiveRef.current) {
        term.write("\r\x1b[K")

        if (data === "\r") {
          const savedPassword = connectionRef.current?.password
          if (savedPassword) {
            invoke("write_pty", { tabId, data: savedPassword + "\n" }).catch(console.error)
            passwordPromptActiveRef.current = false
            return
          }
        }

        passwordPromptActiveRef.current = false
      }

      invoke("write_pty", { tabId, data }).catch(console.error)
    })

    let unlistenOutput: (() => void) | null = null
    let unlistenExit: (() => void) | null = null
    let unlistenHostPrompt: (() => void) | null = null
    let unlistenConnectionProgress: (() => void) | null = null
    let disposed = false

    Promise.all([
      listen<string>(`pty-output-${tabId}`, (event) => {
        const payload = event.payload
        if (payload.includes(STATUS_CONNECTING)) {
          setConnectionState("connecting")
        } else if (connectionRef.current?.type === "ssh" && payload.trim().length > 0) {
          setConnectionState("connected")
        }

        const sudoPasswordPattern = /^\[sudo\] password for ([^:]+):\s*$/im
        const match = payload.match(sudoPasswordPattern)

        if (match && !passwordPromptActiveRef.current) {
          const promptUsername = match[1].trim()
          const savedUsername = connectionRef.current?.username
          const profileId = connectionRef.current?.profileId
          const profileName = connectionRef.current?.profileName

          if (savedUsername && promptUsername === savedUsername && profileName) {
            invoke<string | null>("get_saved_password_for_sudo", {
              profileId,
              profileName,
            })
              .then((password) => {
                if (password) {
                  passwordPromptActiveRef.current = true
                  connectionRef.current = {
                    ...connectionRef.current,
                    password,
                  }
                  const pasteHint =
                    "\x1b[100m\x1b[36m tTerm \x1b[0m " +
                    "\x1b[90mPress Enter to paste saved password\x1b[0m"
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
      listen<SshConnectionProgress>(`ssh-connection-progress-${tabId}`, (event) => {
        setConnectionProgress(event.payload)
        if (event.payload.phase === "ready") {
          setConnectionState("connected")
        } else if (event.payload.phase !== "failed") {
          setConnectionState("connecting")
        }
      }),
    ])
      .then(([unOut, unExit, unHostPrompt, unProgress]) => {
        unlistenOutput = unOut
        unlistenExit = unExit
        unlistenHostPrompt = unHostPrompt
        unlistenConnectionProgress = unProgress

        if (disposed) {
          unlistenOutput?.()
          unlistenExit?.()
          unlistenHostPrompt?.()
          unlistenConnectionProgress?.()
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
          invoke("kill_pty", { tabId }).catch(console.error)
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

      if (activateFitTimerRef.current !== null) {
        window.clearTimeout(activateFitTimerRef.current)
        activateFitTimerRef.current = null
      }

      unlistenOutput?.()
      unlistenExit?.()
      unlistenHostPrompt?.()
      unlistenConnectionProgress?.()
      invoke("kill_pty", { tabId }).catch(console.error)
      searchResultsDisposableRef.current?.dispose()
      searchResultsDisposableRef.current = null
      searchAddonRef.current = null
      term.dispose()
      termRef.current = null
      fitAddonRef.current = null
      initializedRef.current = false
      lastPtySizeRef.current = null
      waitingForReconnectRef.current = false
      passwordPromptActiveRef.current = false
    }
  }, [
    activateFitTimerRef,
    connectionRef,
    containerRef,
    fitAddonRef,
    fitTerminalOnly,
    initializedRef,
    initialCursorStyle,
    initialFontFamily,
    initialFontSize,
    initialScrollbackLines,
    initialTerminalThemeRef,
    isActiveRef,
    lastPtySizeRef,
    onPidChangeRef,
    onReconnectRequestRef,
    passwordPromptActiveRef,
    resizeObserverRef,
    resizeRafRef,
    scheduleFitDuringResize,
    searchAddonRef,
    searchResultsDisposableRef,
    setConnectionState,
    setHostKeyPrompt,
    setConnectionProgress,
    setSearchResults,
    sessionNonce,
    tabId,
    termRef,
    waitingForReconnectRef,
  ])
}
