import "@/components/TerminalTab.css"
import "@xterm/xterm/css/xterm.css"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { FitAddon } from "@xterm/addon-fit"
import {
  SearchAddon,
  type ISearchDecorationOptions,
  type ISearchOptions,
  type ISearchResultChangeEvent,
} from "@xterm/addon-search"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { type IDisposable, Terminal } from "@xterm/xterm"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useTranslation } from "react-i18next"
import {
  ArrowDown,
  ArrowUp,
  CaseSensitive,
  GripHorizontal,
  Regex,
  WholeWord,
  X,
} from "lucide-react"

import { SftpDrawer } from "@/components/SftpDrawer"
import { ConnectionHeader } from "@/components/TerminalTab/ConnectionHeader"
import { HostKeyPromptDialog } from "@/components/TerminalTab/HostKeyPromptDialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
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

type SearchOptionsState = {
  caseSensitive: boolean
  wholeWord: boolean
  regex: boolean
}

type SearchDragState = {
  pointerX: number
  pointerY: number
  positionX: number
  positionY: number
  barRect: DOMRect
  surfaceRect: DOMRect
}

const SEARCH_DECORATIONS: ISearchDecorationOptions = {
  matchBackground: "#facc15",
  matchBorder: "#fde047",
  matchOverviewRuler: "#facc15",
  activeMatchBackground: "#fb923c",
  activeMatchBorder: "#fdba74",
  activeMatchColorOverviewRuler: "#fb923c",
}

const DEFAULT_SEARCH_OPTIONS: SearchOptionsState = {
  caseSensitive: false,
  wholeWord: false,
  regex: false,
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
  const surfaceRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const searchAddonRef = useRef<SearchAddon | null>(null)
  const searchResultsDisposableRef = useRef<IDisposable | null>(null)
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const searchDragStateRef = useRef<SearchDragState | null>(null)
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
  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")
  const [searchOptions, setSearchOptions] = useState<SearchOptionsState>(DEFAULT_SEARCH_OPTIONS)
  const [searchResults, setSearchResults] = useState<ISearchResultChangeEvent>({
    resultIndex: -1,
    resultCount: 0,
  })
  const [searchPosition, setSearchPosition] = useState({ x: 0, y: 0 })
  const [isSearchDragging, setIsSearchDragging] = useState(false)

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

  const buildSearchOptions = useCallback(
    (overrides?: Partial<SearchOptionsState>, incremental = false): ISearchOptions => ({
      ...searchOptions,
      ...overrides,
      incremental,
      decorations: SEARCH_DECORATIONS,
    }),
    [searchOptions]
  )

  const runSearch = useCallback(
    (direction: "next" | "previous" = "next", incremental = false) => {
      const searchAddon = searchAddonRef.current
      const query = searchQuery
      if (!searchAddon) return false

      if (!query) {
        searchAddon.clearDecorations()
        setSearchResults({ resultIndex: -1, resultCount: 0 })
        return false
      }

      try {
        const options = buildSearchOptions(undefined, incremental)
        return direction === "previous"
          ? searchAddon.findPrevious(query, options)
          : searchAddon.findNext(query, options)
      } catch (error) {
        console.error("Terminal search failed:", error)
        setSearchResults({ resultIndex: -1, resultCount: 0 })
        return false
      }
    },
    [buildSearchOptions, searchQuery]
  )

  const closeSearch = useCallback(() => {
    searchAddonRef.current?.clearDecorations()
    setIsSearchOpen(false)
    setSearchQuery("")
    setSearchResults({ resultIndex: -1, resultCount: 0 })
    window.setTimeout(() => termRef.current?.focus(), 0)
  }, [])

  const openSearch = useCallback(() => {
    setIsSearchOpen(true)
    setShowSftpDrawer(false)
    window.setTimeout(() => {
      searchInputRef.current?.focus()
      searchInputRef.current?.select()
    }, 0)
  }, [])

  const handleSearchDragStart = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) return

      const bar = event.currentTarget.closest<HTMLDivElement>(".terminal-search-bar")
      const surface = surfaceRef.current
      if (!bar || !surface) return

      event.preventDefault()
      searchDragStateRef.current = {
        pointerX: event.clientX,
        pointerY: event.clientY,
        positionX: searchPosition.x,
        positionY: searchPosition.y,
        barRect: bar.getBoundingClientRect(),
        surfaceRect: surface.getBoundingClientRect(),
      }
      setIsSearchDragging(true)
    },
    [searchPosition.x, searchPosition.y]
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

  useEffect(() => {
    if (!isSearchOpen) return

    window.setTimeout(() => {
      searchInputRef.current?.focus()
      searchInputRef.current?.select()
    }, 0)
  }, [isSearchOpen])

  useEffect(() => {
    if (!isSearchOpen) return

    const searchTimer = window.setTimeout(() => {
      runSearch("next", true)
    }, 0)

    return () => window.clearTimeout(searchTimer)
  }, [isSearchOpen, runSearch])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isActiveRef.current) return

      const isFindShortcut = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f"
      if (!isFindShortcut) return

      event.preventDefault()
      openSearch()
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [openSearch])

  useEffect(() => {
    if (!isSearchDragging) return

    const margin = 8
    const handlePointerMove = (event: PointerEvent) => {
      const dragState = searchDragStateRef.current
      if (!dragState) return

      const deltaX = event.clientX - dragState.pointerX
      const deltaY = event.clientY - dragState.pointerY
      const minDeltaX = dragState.surfaceRect.left + margin - dragState.barRect.left
      const maxDeltaX = dragState.surfaceRect.right - margin - dragState.barRect.right
      const minDeltaY = dragState.surfaceRect.top + margin - dragState.barRect.top
      const maxDeltaY = dragState.surfaceRect.bottom - margin - dragState.barRect.bottom

      setSearchPosition({
        x: dragState.positionX + Math.min(Math.max(deltaX, minDeltaX), maxDeltaX),
        y: dragState.positionY + Math.min(Math.max(deltaY, minDeltaY), maxDeltaY),
      })
    }

    const handlePointerUp = () => {
      searchDragStateRef.current = null
      setIsSearchDragging(false)
    }

    window.addEventListener("pointermove", handlePointerMove)
    window.addEventListener("pointerup", handlePointerUp, { once: true })
    return () => {
      window.removeEventListener("pointermove", handlePointerMove)
      window.removeEventListener("pointerup", handlePointerUp)
    }
  }, [isSearchDragging])

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
      allowProposedApi: true,
    })

    const fitAddon = new FitAddon()
    const searchAddon = new SearchAddon({ highlightLimit: 2000 })
    term.loadAddon(fitAddon)
    term.loadAddon(searchAddon)
    term.loadAddon(new WebLinksAddon())

    termRef.current = term
    fitAddonRef.current = fitAddon
    searchAddonRef.current = searchAddon
    searchResultsDisposableRef.current = searchAddon.onDidChangeResults((results) => {
      setSearchResults(results)
    })

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

  const toggleSearchOption = useCallback((option: keyof SearchOptionsState) => {
    setSearchOptions((current) => ({
      ...current,
      [option]: !current[option],
    }))
  }, [])

  const handleSearchKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Escape") {
        event.preventDefault()
        closeSearch()
        return
      }

      if (event.key === "Enter") {
        event.preventDefault()
        runSearch(event.shiftKey ? "previous" : "next")
      }
    },
    [closeSearch, runSearch]
  )

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

  const searchResultText = searchQuery
    ? searchResults.resultCount > 0
      ? t("terminalSearch.results", {
          current: searchResults.resultIndex >= 0 ? searchResults.resultIndex + 1 : 0,
          total: searchResults.resultCount,
          defaultValue: "{{current}} / {{total}}",
        })
      : t("terminalSearch.noResults", { defaultValue: "No results" })
    : t("terminalSearch.ready", { defaultValue: "Find in terminal" })

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

      <div ref={surfaceRef} className="terminal-surface">
        <SftpDrawer
          tabId={tabId}
          visible={showSftpDrawer}
          connection={connection}
          onClose={() => setShowSftpDrawer(false)}
        />

        {isSearchOpen && (
          <div
            className="terminal-search-bar"
            data-dragging={isSearchDragging ? "true" : undefined}
            onMouseDown={(event) => event.stopPropagation()}
            style={{ transform: `translate(${searchPosition.x}px, ${searchPosition.y}px)` }}
          >
            <div
              className="terminal-search-drag-handle"
              onPointerDown={handleSearchDragStart}
              title={t("terminalSearch.drag", { defaultValue: "Drag search box" })}
              aria-label={t("terminalSearch.drag", { defaultValue: "Drag search box" })}
            >
              <GripHorizontal />
            </div>
            <Input
              ref={searchInputRef}
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              onKeyDown={handleSearchKeyDown}
              placeholder={t("terminalSearch.placeholder", { defaultValue: "Search terminal" })}
              aria-label={t("terminalSearch.placeholder", { defaultValue: "Search terminal" })}
              className="terminal-search-input"
            />
            <span
              className="terminal-search-count"
              aria-live="polite"
              data-empty={searchQuery ? undefined : "true"}
              data-missing={searchQuery && searchResults.resultCount === 0 ? "true" : undefined}
            >
              {searchResultText}
            </span>
            <Button
              type="button"
              variant={searchOptions.caseSensitive ? "secondary" : "ghost"}
              size="icon-xs"
              onClick={() => toggleSearchOption("caseSensitive")}
              title={t("terminalSearch.caseSensitive", { defaultValue: "Match case" })}
              aria-pressed={searchOptions.caseSensitive}
            >
              <CaseSensitive />
            </Button>
            <Button
              type="button"
              variant={searchOptions.wholeWord ? "secondary" : "ghost"}
              size="icon-xs"
              onClick={() => toggleSearchOption("wholeWord")}
              title={t("terminalSearch.wholeWord", { defaultValue: "Whole word" })}
              aria-pressed={searchOptions.wholeWord}
            >
              <WholeWord />
            </Button>
            <Button
              type="button"
              variant={searchOptions.regex ? "secondary" : "ghost"}
              size="icon-xs"
              onClick={() => toggleSearchOption("regex")}
              title={t("terminalSearch.regex", { defaultValue: "Use regular expression" })}
              aria-pressed={searchOptions.regex}
            >
              <Regex />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={() => runSearch("previous")}
              title={t("terminalSearch.previous", { defaultValue: "Previous result" })}
            >
              <ArrowUp />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={() => runSearch("next")}
              title={t("terminalSearch.next", { defaultValue: "Next result" })}
            >
              <ArrowDown />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={closeSearch}
              title={t("terminalSearch.close", { defaultValue: "Close search" })}
            >
              <X />
            </Button>
          </div>
        )}

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
