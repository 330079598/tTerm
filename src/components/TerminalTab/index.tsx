import "@/components/TerminalTab.css"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { FitAddon } from "@xterm/addon-fit"
import { SearchAddon } from "@xterm/addon-search"
import { type IDisposable, Terminal } from "@xterm/xterm"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"
import { Pause, Play, Square } from "lucide-react"
import { SftpDrawer } from "@/components/SftpDrawer"
import { ConnectionHeader } from "@/components/TerminalTab/ConnectionHeader"
import { HostKeyPromptDialog } from "@/components/TerminalTab/HostKeyPromptDialog"
import { JumpHostInfoDialog } from "@/components/TerminalTab/JumpHostInfoDialog"
import { ServerMonitorBar } from "@/components/TerminalTab/ServerMonitorBar"
import { TerminalSearchBar } from "@/components/TerminalTab/TerminalSearchBar"
import { useTerminalSearch } from "@/components/TerminalTab/useTerminalSearch"
import { useTerminalLifecycle } from "@/components/TerminalTab/useTerminalLifecycle"
import { TAB_ACTIVATE_REFIT_DELAY_MS } from "@/components/TerminalTab/terminalTabUtils"
import type {
  ConnectionState,
  HostKeyPromptState,
  SshConnectionProgress,
  TerminalTabProps,
} from "@/components/TerminalTab/types"
import { toast } from "@/hooks/use-toast"
import { useConfig } from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"
import { useStableRef } from "@/hooks/useStableRef"
import { resolveScrollbackLines } from "@/lib/scrollback"
import { toErrorMessage } from "@/lib/utils"

const FIT_STABILITY_FRAMES = 2
const MAX_PENDING_FIT_FRAMES = 4

export const TerminalTab: React.FC<TerminalTabProps> = ({
  tabId,
  sessionNonce = 0,
  isActive,
  workspacePanelApi,
  connectionHeaderPinned = true,
  connection,
  isBroadcastSource = false,
  liveBroadcastState = "idle",
  onConnectionStateChange,
  onInput,
  onPidChange,
  onReconnectRequest,
  onSessionUnavailable,
  onSensitivePrompt,
  onOpenRemoteFile,
  onPinConnectionHeader,
  onPauseBroadcast,
  onResumeBroadcast,
  onStopBroadcast,
  onServerMonitorVisibilityChange,
  onUnpinConnectionHeader,
}) => {
  const containerRef = useRef<HTMLDivElement>(null)
  const surfaceRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const searchAddonRef = useRef<SearchAddon | null>(null)
  const searchResultsDisposableRef = useRef<IDisposable | null>(null)
  const resizeObserverRef = useRef<ResizeObserver | null>(null)
  const resizeRafRef = useRef<number | null>(null)
  const resizePtySyncTimerRef = useRef<number | null>(null)
  const activateFitTimerRef = useRef<number | null>(null)
  const lastPtySizeRef = useRef<{ rows: number; cols: number } | null>(null)
  const connectionRef = useStableRef(connection)
  const isActiveRef = useStableRef(isActive)
  const initializedRef = useRef(false)
  const creatingPtyRef = useRef(false)
  const waitingForReconnectRef = useRef(false)
  const onPidChangeRef = useStableRef(onPidChange)
  const onInputRef = useStableRef(onInput)
  const onReconnectRequestRef = useStableRef(onReconnectRequest)
  const onSessionUnavailableRef = useStableRef(onSessionUnavailable)
  const onSensitivePromptRef = useStableRef(onSensitivePrompt)
  const { config, saveConfig } = useConfig()
  const { currentTheme, getTheme } = useTheme()
  const { t } = useTranslation()
  const initialFontFamily = useRef(config.font_family)
  const initialFontSize = useRef(config.font_size)
  const initialCursorStyle = useRef(config.cursor_style)
  const initialScrollbackLines = useRef(config.scrollback_lines)
  const sessionResetKey = `${tabId}:${sessionNonce}:${connection?.type ?? "terminal"}`
  const defaultConnectionState: ConnectionState = "connecting"
  const passwordPromptActiveRef = useRef(false)
  const lastJumpHostReadyKeyRef = useRef<string | null>(null)

  const [hostKeyPromptState, setHostKeyPromptState] = useState<{
    sessionKey: string
    value: HostKeyPromptState
  } | null>(null)
  const [connectionStateState, setConnectionStateState] = useState<{
    sessionKey: string
    value: ConnectionState
  } | null>(null)
  const [connectionProgressState, setConnectionProgressState] = useState<{
    sessionKey: string
    value: SshConnectionProgress
  } | null>(null)
  const [showSftpDrawer, setShowSftpDrawer] = useState(false)
  const [showServerMonitor, setShowServerMonitor] = useState(
    () => connection?.serverMonitorVisible === true
  )
  const [jumpHostInfoOpen, setJumpHostInfoOpen] = useState(false)
  const [dontShowJumpHostInfoAgain, setDontShowJumpHostInfoAgain] = useState(false)

  const {
    closeSearch,
    handleSearchDragStart,
    isSearchDragging,
    isSearchOpen,
    runSearch,
    searchInputRef,
    searchOptions,
    searchPosition,
    searchQuery,
    searchResults,
    setSearchQuery,
    setSearchResults,
    toggleSearchOption,
  } = useTerminalSearch({
    isActiveRef,
    searchAddonRef,
    setShowSftpDrawer,
    surfaceRef,
    termRef,
  })

  const hostKeyPrompt =
    hostKeyPromptState?.sessionKey === sessionResetKey ? hostKeyPromptState.value : null
  const connectionState =
    connectionStateState?.sessionKey === sessionResetKey
      ? connectionStateState.value
      : defaultConnectionState
  const connectionProgress =
    connectionProgressState?.sessionKey === sessionResetKey ? connectionProgressState.value : null

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

  const setConnectionProgress = useCallback(
    (value: SshConnectionProgress | null) => {
      setConnectionProgressState(value ? { sessionKey: sessionResetKey, value } : null)
    },
    [sessionResetKey]
  )

  const resolveTerminalTheme = useCallback(() => {
    return { ...(getTheme(currentTheme)?.terminal ?? getTheme("default")!.terminal) }
  }, [currentTheme, getTheme])

  // Keep the first palette for terminal creation; later theme changes update xterm in place.
  const initialTerminalThemeRef =
    useRef<ReturnType<typeof resolveTerminalTheme>>(resolveTerminalTheme())

  const fitTerminalOnly = useCallback(() => {
    const fitAddon = fitAddonRef.current
    const term = termRef.current
    if (!fitAddon || !term) return false

    const dimensions = fitAddon.proposeDimensions()
    if (!dimensions) return false

    fitAddon.fit()
    return term.cols === dimensions.cols && term.rows === dimensions.rows
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
      invoke("resize_pty", {
        tabId,
        sessionNonce,
        rows: nextSize.rows,
        cols: nextSize.cols,
      }).catch(console.error)
    },
    [sessionNonce, tabId]
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
    if (resizePtySyncTimerRef.current !== null) {
      window.clearTimeout(resizePtySyncTimerRef.current)
      resizePtySyncTimerRef.current = null
    }

    let frameCount = 0
    let stableFrameCount = 0
    const fitAfterLayout = () => {
      resizeRafRef.current = window.requestAnimationFrame(() => {
        resizeRafRef.current = null
        if (!isActiveRef.current) return

        frameCount += 1
        const fitted = fitTerminalOnly()
        stableFrameCount = fitted ? stableFrameCount + 1 : 0

        if (stableFrameCount < FIT_STABILITY_FRAMES && frameCount < MAX_PENDING_FIT_FRAMES) {
          fitAfterLayout()
          return
        }

        if (!fitted) return

        const term = termRef.current
        if (term && term.rows > 0) {
          term.refresh(0, term.rows - 1)
        }

        resizePtySyncTimerRef.current = window.setTimeout(() => {
          resizePtySyncTimerRef.current = null
          if (isActiveRef.current) {
            syncPtySize()
          }
        }, 80)
      })
    }

    fitAfterLayout()
  }, [fitTerminalOnly, isActiveRef, syncPtySize])

  useEffect(() => {
    const term = termRef.current
    if (!term) return

    term.options.fontFamily = config.font_family
    term.options.fontSize = config.font_size
    term.options.cursorStyle = config.cursor_style
    if (isActiveRef.current) {
      scheduleFitDuringResize()
    }
  }, [
    config.cursor_style,
    config.font_family,
    config.font_size,
    isActiveRef,
    scheduleFitDuringResize,
  ])

  useEffect(() => {
    const term = termRef.current
    if (!term) return

    // 0 = unlimited in settings; resolveScrollbackLines maps it for xterm.
    term.options.scrollback = resolveScrollbackLines(config.scrollback_lines)
  }, [config.scrollback_lines])

  useEffect(() => {
    if (!isActiveRef.current) return

    fitAndSyncPty()
  }, [
    config.terminal_padding_bottom_px,
    config.terminal_padding_left_px,
    config.terminal_padding_right_px,
    fitAndSyncPty,
    isActiveRef,
  ])

  useEffect(() => {
    const term = termRef.current
    if (!term) return

    term.options.theme = resolveTerminalTheme()
  }, [currentTheme, resolveTerminalTheme])

  useTerminalLifecycle({
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
    onInputRef,
    onReconnectRequestRef,
    onSessionUnavailableRef,
    onSensitivePromptRef,
    passwordPromptActiveRef,
    resizeObserverRef,
    resizePtySyncTimerRef,
    resizeRafRef,
    scheduleFitDuringResize,
    surfaceRef,
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
  })

  useEffect(() => {
    onConnectionStateChange?.(tabId, sessionNonce, connectionState)
  }, [connectionState, onConnectionStateChange, sessionNonce, tabId])

  useEffect(
    () => () => onConnectionStateChange?.(tabId, sessionNonce, null),
    [onConnectionStateChange, sessionNonce, tabId]
  )

  useEffect(() => {
    if (!workspacePanelApi) return

    const disposable = workspacePanelApi.onDidDimensionsChange(() => {
      if (isActiveRef.current) {
        scheduleFitDuringResize()
      }
    })

    return () => disposable.dispose()
  }, [isActiveRef, scheduleFitDuringResize, workspacePanelApi])

  useEffect(() => {
    const container = containerRef.current
    const surface = surfaceRef.current
    const resizeObserver = resizeObserverRef.current
    if (!container || !resizeObserver) return

    if (isActive) {
      resizeObserver.observe(container)
      if (surface) {
        resizeObserver.observe(surface)
      }
      activateFitTimerRef.current = window.setTimeout(() => {
        activateFitTimerRef.current = null
        scheduleFitDuringResize()
        termRef.current?.focus()
      }, TAB_ACTIVATE_REFIT_DELAY_MS)
      return
    }

    resizeObserver.unobserve(container)
    if (surface) {
      resizeObserver.unobserve(surface)
    }

    if (resizeRafRef.current !== null) {
      window.cancelAnimationFrame(resizeRafRef.current)
      resizeRafRef.current = null
    }

    if (activateFitTimerRef.current !== null) {
      window.clearTimeout(activateFitTimerRef.current)
      activateFitTimerRef.current = null
    }
  }, [isActive, scheduleFitDuringResize])

  const jumpHostCount = connection?.jumpHosts?.length ?? 0

  useEffect(() => {
    if (
      connectionProgress?.phase !== "ready" ||
      connection?.type !== "ssh" ||
      jumpHostCount === 0
    ) {
      return
    }

    const readyKey = `${sessionResetKey}:${jumpHostCount}`
    if (lastJumpHostReadyKeyRef.current === readyKey) {
      return
    }
    lastJumpHostReadyKeyRef.current = readyKey

    if (config.show_jump_host_connection_info) {
      const openDialogTimer = window.setTimeout(() => {
        setDontShowJumpHostInfoAgain(false)
        setJumpHostInfoOpen(true)
      }, 0)
      return () => window.clearTimeout(openDialogTimer)
    }

    toast({
      title: t("jumpHostInfo.toastTitle", { defaultValue: "Jump host route ready" }),
      description: t("jumpHostInfo.toastDescription", {
        count: jumpHostCount,
        defaultValue: "Connected through {{count}} jump host(s).",
      }),
    })
  }, [
    config.show_jump_host_connection_info,
    connection,
    connectionProgress?.phase,
    jumpHostCount,
    sessionResetKey,
    t,
  ])

  const handleJumpHostInfoOpenChange = useCallback(
    (open: boolean) => {
      setJumpHostInfoOpen(open)
      if (open || !dontShowJumpHostInfoAgain) {
        return
      }

      saveConfig({ show_jump_host_connection_info: false }).catch((error) => {
        console.error("Failed to save jump host info preference:", error)
        toast({
          title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      })
    },
    [dontShowJumpHostInfoAgain, saveConfig, t]
  )

  const handleReconnect = useCallback(() => {
    onSessionUnavailable?.(tabId)
    waitingForReconnectRef.current = false
    setConnectionState("connecting")
    onReconnectRequest?.()
  }, [onReconnectRequest, onSessionUnavailable, setConnectionState, tabId])

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

  const handleToggleServerMonitor = useCallback(() => {
    setShowServerMonitor((current) => {
      const nextVisible = !current
      onServerMonitorVisibilityChange?.(nextVisible)
      return nextVisible
    })
  }, [onServerMonitorVisibilityChange])

  const handleSearchKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape") {
        event.preventDefault()
        event.stopPropagation()
        closeSearch()
        return
      }

      if (event.key === "Enter" && event.target === searchInputRef.current) {
        event.preventDefault()
        runSearch(event.shiftKey ? "previous" : "next")
      }
    },
    [closeSearch, runSearch, searchInputRef]
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

        if (onInput) {
          await onInput({ tabId, sessionNonce, data: clipboardText, kind: "paste" })
        } else {
          await invoke("write_pty", { tabId, sessionNonce, data: clipboardText })
        }
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
    [onInput, sessionNonce, showSftpDrawer, t, tabId]
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

  const terminalPaddingStyle = {
    "--terminal-padding-left": `${config.terminal_padding_left_px}px`,
    "--terminal-padding-right": `${config.terminal_padding_right_px}px`,
    "--terminal-padding-bottom": `${config.terminal_padding_bottom_px}px`,
  } as React.CSSProperties

  return (
    <div
      className={`terminal-tab-shell ${isBroadcastSource ? "is-broadcast-source" : ""}`}
      aria-label={isBroadcastSource ? t("broadcast.sourceTerminal") : undefined}
    >
      <ConnectionHeader
        connection={connection}
        connectionHeaderPinned={connectionHeaderPinned}
        connectionState={connectionState}
        connectionProgress={connectionProgress}
        onBackgroundMouseDown={handleConnectionHeaderMouseDown}
        onPinConnectionHeader={onPinConnectionHeader}
        onReconnect={handleReconnect}
        onToggleServerMonitor={handleToggleServerMonitor}
        onToggleSftpDrawer={handleToggleSftpDrawer}
        onUnpinConnectionHeader={onUnpinConnectionHeader}
        serverMonitorVisible={showServerMonitor}
      />

      {isBroadcastSource && liveBroadcastState !== "idle" && (
        <div
          className={`terminal-broadcast-controls is-${liveBroadcastState}`}
          style={{ fontFamily: config.font_family }}
        >
          <span role="status">
            {liveBroadcastState === "paused"
              ? t("broadcast.livePaused")
              : t("broadcast.liveActive")}
          </span>
          <div>
            {liveBroadcastState === "paused" ? (
              <button type="button" onClick={onResumeBroadcast}>
                <Play size={13} aria-hidden="true" />
                {t("broadcast.resumeLive")}
              </button>
            ) : (
              <button type="button" onClick={onPauseBroadcast}>
                <Pause size={13} aria-hidden="true" />
                {t("broadcast.pauseLive")}
              </button>
            )}
            <button type="button" className="destructive" onClick={onStopBroadcast}>
              <Square size={13} aria-hidden="true" />
              {t("broadcast.stopLive")}
            </button>
          </div>
        </div>
      )}

      <div ref={surfaceRef} className="terminal-surface" style={terminalPaddingStyle}>
        <SftpDrawer
          tabId={tabId}
          visible={showSftpDrawer}
          connection={connection}
          onClose={() => setShowSftpDrawer(false)}
          onOpenRemoteFile={onOpenRemoteFile}
        />
        {isSearchOpen && (
          <TerminalSearchBar
            isSearchDragging={isSearchDragging}
            onClose={closeSearch}
            onDragStart={handleSearchDragStart}
            onKeyDown={handleSearchKeyDown}
            onRunSearch={(direction) => runSearch(direction)}
            onToggleOption={toggleSearchOption}
            searchInputRef={searchInputRef}
            searchOptions={searchOptions}
            searchPosition={searchPosition}
            searchQuery={searchQuery}
            searchResultText={searchResultText}
            searchResults={searchResults}
            setSearchQuery={setSearchQuery}
            t={t}
          />
        )}

        <div
          ref={containerRef}
          data-allow-context-menu
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
        <JumpHostInfoDialog
          connection={connection}
          dontShowAgain={dontShowJumpHostInfoAgain}
          onDontShowAgainChange={setDontShowJumpHostInfoAgain}
          onOpenChange={handleJumpHostInfoOpenChange}
          open={jumpHostInfoOpen}
        />
      </div>
      <ServerMonitorBar
        connection={connection}
        connectionState={connectionState}
        sessionNonce={sessionNonce}
        tabId={tabId}
        t={t}
        visible={showServerMonitor}
      />
    </div>
  )
}
