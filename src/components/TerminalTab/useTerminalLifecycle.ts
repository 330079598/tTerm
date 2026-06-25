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
  creatingPtyRef: React.MutableRefObject<boolean>
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
  creatingPtyRef,
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

    /**
     * xterm v6 floating scrollbar: PowerShell-style auto-thin behavior.
     *
     * Goal: scrollbar is always visible — a thin dim line when the mouse is
     * outside the terminal, expanding to full width when the mouse enters.
     *
     * xterm adds the `.invisible` class on mouse leave and removes it on mouse
     * enter, with its own CSS setting `opacity: 0; pointer-events: none`.
     * Our CSS overrides in xterm-overrides.css handle colors/transitions and
     * work in dev, but in production builds Tailwind v4's layer ordering can
     * demote those rules. This JS layer is the reliable enforcement: it keeps
     * the scrollbar visible (inline !important beats any stylesheet) and
     * toggles width based on hover state.
     */
    let mouseInside = false
    const applyScrollbarWidth = (wide: boolean) => {
      // Target ONLY the vertical scrollbar. xterm v6 appends the horizontal
      // scrollbar first, then the vertical one, so a bare ".scrollbar"
      // selector would match the horizontal one and force it visible,
      // producing a stray vertical bar in the bottom-left corner.
      const scrollbar = container.querySelector<HTMLElement>(
        ".xterm-scrollable-element > .scrollbar.vertical"
      )
      if (!scrollbar) return
      // xterm reuses the class name "invisible" for its idle scrollbar state.
      // Tailwind v4 generates `.invisible { visibility: hidden !important }`
      // as a utility, which clashes with xterm's class and fully hides the
      // scrollbar in production. We override with inline !important styles,
      // which beat any stylesheet rule including Tailwind utilities.
      scrollbar.style.setProperty("visibility", "visible", "important")
      scrollbar.style.setProperty("opacity", "1", "important")
      scrollbar.style.setProperty("pointer-events", "auto", "important")
      scrollbar.style.setProperty("display", "block", "important")
      scrollbar.style.setProperty("width", wide ? "10px" : "4px", "important")
      const slider = scrollbar.querySelector<HTMLElement>(".slider")
      if (slider) {
        slider.style.setProperty("visibility", "visible", "important")
        slider.style.setProperty("width", wide ? "10px" : "4px", "important")
      }
    }

    // Start thin (idle) — the terminal container starts without mouse.
    requestAnimationFrame(() => applyScrollbarWidth(false))

    // Mouse enters the terminal surface → wide scrollbar.
    const handleMouseEnter = () => {
      mouseInside = true
      applyScrollbarWidth(true)
    }
    // Mouse leaves the terminal surface → thin scrollbar.
    const handleMouseLeave = () => {
      mouseInside = false
      applyScrollbarWidth(false)
    }
    container.addEventListener("mouseenter", handleMouseEnter)
    container.addEventListener("mouseleave", handleMouseLeave)

    // Safety net: if xterm toggles .invisible or recreates the scrollbar,
    // re-apply the current target width so it never disappears in production.
    const scrollableEl = container.querySelector(".xterm-scrollable-element")
    let classObserver: MutationObserver | null = null
    if (scrollableEl) {
      classObserver = new MutationObserver(() => {
        applyScrollbarWidth(mouseInside)
      })
      classObserver.observe(scrollableEl, {
        subtree: true,
        attributes: true,
        attributeFilter: ["class"],
      })
    }

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
          const profileId = connectionRef.current?.profileId
          const profileName = connectionRef.current?.profileName
          passwordPromptActiveRef.current = false
          invoke("write_saved_password_for_sudo", {
            tabId,
            sessionNonce,
            profileId,
            profileName,
          }).catch(console.error)
          return
        }

        passwordPromptActiveRef.current = false
      }

      invoke("write_pty", { tabId, sessionNonce, data }).catch(console.error)
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
            invoke<boolean>("has_saved_password", {
              profileId,
              profileName,
            })
              .then((hasPassword) => {
                if (hasPassword) {
                  passwordPromptActiveRef.current = true
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

        if (creatingPtyRef.current) {
          return null
        }

        creatingPtyRef.current = true
        return invoke<number>("create_pty", {
          tabId,
          sessionNonce,
          rows: term.rows,
          cols: term.cols,
          connection: connectionRef.current,
        })
      })
      .then((pid) => {
        if (pid == null) return

        if (disposed) {
          invoke("kill_pty", { tabId, sessionNonce }).catch(console.error)
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
      .finally(() => {
        creatingPtyRef.current = false
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

      container.removeEventListener("mouseenter", handleMouseEnter)
      container.removeEventListener("mouseleave", handleMouseLeave)
      classObserver?.disconnect()

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
      invoke("kill_pty", { tabId, sessionNonce }).catch(console.error)
      searchResultsDisposableRef.current?.dispose()
      searchResultsDisposableRef.current = null
      searchAddonRef.current = null
      term.dispose()
      termRef.current = null
      fitAddonRef.current = null
      initializedRef.current = false
      lastPtySizeRef.current = null
      creatingPtyRef.current = false
      waitingForReconnectRef.current = false
      passwordPromptActiveRef.current = false
    }
  }, [
    activateFitTimerRef,
    connectionRef,
    containerRef,
    creatingPtyRef,
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
