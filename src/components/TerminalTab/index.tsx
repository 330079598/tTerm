import "@/components/TerminalTab.css"
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { FitAddon } from "@xterm/addon-fit"
import { SearchAddon } from "@xterm/addon-search"
import { type IDisposable, Terminal } from "@xterm/xterm"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"
import { Keyboard, Pause, Play, Square } from "lucide-react"
import { ContextMenu } from "@/components/ContextMenu"
import { SftpDrawer } from "@/components/SftpDrawer"
import { ConnectionHeader } from "@/components/TerminalTab/ConnectionHeader"
import { HostKeyPromptDialog } from "@/components/TerminalTab/HostKeyPromptDialog"
import { JumpHostInfoDialog } from "@/components/TerminalTab/JumpHostInfoDialog"
import { SavedPasswordPromptBar } from "@/components/TerminalTab/SavedPasswordPromptBar"
import { ServerMonitorBar } from "@/components/TerminalTab/ServerMonitorBar"
import { TerminalSearchBar } from "@/components/TerminalTab/TerminalSearchBar"
import { useTerminalSearch } from "@/components/TerminalTab/useTerminalSearch"
import { useTerminalLifecycle } from "@/components/TerminalTab/useTerminalLifecycle"
import { useZmodemTransfers } from "@/components/TerminalTab/useZmodemTransfers"
import { TAB_ACTIVATE_REFIT_DELAY_MS } from "@/components/TerminalTab/terminalTabUtils"
import type {
  ConnectionState,
  HostKeyPromptState,
  SavedPasswordPromptActions,
  SavedPasswordPromptState,
  SshConnectionProgress,
  TerminalTabProps,
} from "@/components/TerminalTab/types"
import { toast } from "@/hooks/use-toast"
import { useConfig } from "@/contexts/ConfigContext"
import type { TerminalRenderer } from "@/contexts/ConfigContext"
import { useKeymap } from "@/contexts/KeymapContext"
import { useTheme } from "@/contexts/ThemeContext"
import { useStableRef } from "@/hooks/useStableRef"
import { resolveScrollbackLines } from "@/lib/scrollback"
import { safePreloadFont, updateCanvasFontHostFont } from "@/lib/canvasFontHost"
import { compilePromptPatterns } from "@/lib/sudoPrompt"
import { toErrorMessage } from "@/lib/utils"
import type { TabContextMenuAction } from "@/types/tab"

const FIT_STABILITY_FRAMES = 2
const MAX_PENDING_FIT_FRAMES = 4

export const TerminalTab: React.FC<TerminalTabProps> = ({
  tabId,
  sessionNonce = 0,
  isActive,
  isGlobalShortcutTarget,
  workspacePanelApi,
  connectionHeaderPinned = true,
  connection,
  isBroadcastSource = false,
  liveBroadcastState = "idle",
  onConnectionStateChange,
  onInput,
  onCommandExecuted,
  onOpenCommandLibrary,
  onSaveCommand,
  onPidChange,
  onReconnectRequest,
  onSavedPasswordPromptChange,
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
  const onCommandExecutedRef = useStableRef(onCommandExecuted)
  const onReconnectRequestRef = useStableRef(onReconnectRequest)
  const onSavedPasswordPromptChangeRef = useStableRef(onSavedPasswordPromptChange)
  const onSessionUnavailableRef = useStableRef(onSessionUnavailable)
  const onSensitivePromptRef = useStableRef(onSensitivePrompt)
  const { config, saveConfig } = useConfig()
  const { currentTheme, getTheme } = useTheme()
  const { t } = useTranslation()
  const configFontFamilyRef = useStableRef(config.font_family)
  const configFontSizeRef = useStableRef(config.font_size)
  const configCursorStyleRef = useStableRef(config.cursor_style)
  const configScrollbackLinesRef = useStableRef(config.scrollback_lines)
  const configTerminalRendererRef = useStableRef<TerminalRenderer>(config.terminal_renderer)
  const sessionResetKey = `${tabId}:${sessionNonce}:${connection?.type ?? "terminal"}`
  const defaultConnectionState: ConnectionState = "connecting"
  const savedPasswordPromptActionsRef = useRef<SavedPasswordPromptActions | null>(null)
  const [savedPasswordPrompt, setSavedPasswordPrompt] = useState<SavedPasswordPromptState | null>(
    null
  )
  const sudoPromptPatterns = useMemo(
    () => compilePromptPatterns(config.sudo_prompt_patterns),
    [config.sudo_prompt_patterns]
  )
  const sudoPromptPatternsRef = useStableRef(sudoPromptPatterns)
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
  const [terminalContextMenu, setTerminalContextMenu] = useState<{
    x: number
    y: number
    selection: string
  } | null>(null)

  const {
    closeSearch,
    handleSearchDragStart,
    isSearchDragging,
    isSearchOpen,
    openSearch,
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

  const { bindings, registerHandler } = useKeymap()

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

  const terminalTheme = useMemo(
    () => ({ ...(getTheme(currentTheme)?.terminal ?? getTheme("default")!.terminal) }),
    [currentTheme, getTheme]
  )
  const terminalThemeRef = useStableRef(terminalTheme)

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

  useTerminalLifecycle({
    activateFitTimerRef,
    connectionRef,
    containerRef,
    creatingPtyRef,
    fitAddonRef,
    fitTerminalOnly,
    initializedRef,
    configCursorStyleRef,
    configFontFamilyRef,
    configFontSizeRef,
    configScrollbackLinesRef,
    configTerminalRendererRef,
    terminalThemeRef,
    isActiveRef,
    lastPtySizeRef,
    onPidChangeRef,
    onInputRef,
    onCommandExecutedRef,
    onReconnectRequestRef,
    onSavedPasswordPromptChangeRef,
    onSessionUnavailableRef,
    onSensitivePromptRef,
    savedPasswordPromptActionsRef,
    setSavedPasswordPrompt,
    sudoPromptPatternsRef,
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
    terminalRenderer: config.terminal_renderer,
    termRef,
    waitingForReconnectRef,
  })

  useZmodemTransfers({ tabId, sessionNonce })

  const lastAppliedFontRef = useRef<{ family: string; size: number } | null>(null)

  useEffect(() => {
    const term = termRef.current
    if (!term) return

    term.options.cursorStyle = config.cursor_style

    const fontChanged =
      !lastAppliedFontRef.current ||
      lastAppliedFontRef.current.family !== config.font_family ||
      lastAppliedFontRef.current.size !== config.font_size

    if (fontChanged) {
      lastAppliedFontRef.current = {
        family: config.font_family,
        size: config.font_size,
      }
      updateCanvasFontHostFont(config.font_family, config.font_size)
      term.options.fontFamily = config.font_family
      term.options.fontSize = config.font_size

      if (isActiveRef.current) {
        scheduleFitDuringResize()
        term.refresh(0, Math.max(0, term.rows - 1))
      }

      void safePreloadFont(config.font_size, config.font_family).then((loaded) => {
        if (loaded && isActiveRef.current) {
          scheduleFitDuringResize()
          term.refresh(0, Math.max(0, term.rows - 1))
        }
      })
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
  }, [config.scrollback_lines, sessionNonce])

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
  }, [currentTheme, resolveTerminalTheme, sessionNonce])

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
    onSessionUnavailable?.(tabId, sessionNonce, false)
    waitingForReconnectRef.current = false
    setConnectionState("connecting")
    onReconnectRequest?.()
  }, [onReconnectRequest, onSessionUnavailable, sessionNonce, setConnectionState, tabId])

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
    (event: React.MouseEvent<HTMLDivElement>) => {
      event.preventDefault()
      const term = termRef.current
      term?.focus()
      if (showSftpDrawer) setShowSftpDrawer(false)
      setTerminalContextMenu({
        x: event.clientX,
        y: event.clientY,
        selection: term?.hasSelection() ? term.getSelection() : "",
      })
    },
    [showSftpDrawer]
  )

  const clearTerminalHistory = useCallback(() => {
    const term = termRef.current
    if (!term) return

    term.clear()
    term.scrollToBottom()
    term.clearSelection()
    term.focus()
  }, [])

  const armZmodemManualTrigger = useCallback(
    async (direction: "send" | "receive") => {
      try {
        await invoke("zmodem_arm_manual_detect", { tabId, sessionNonce })
        toast({
          title: t(`zmodem.manual.${direction}ArmedTitle`),
          description: t(`zmodem.manual.${direction}ArmedDescription`),
        })
      } catch (error) {
        console.error("Failed to arm ZMODEM manual trigger:", error)
        toast({
          title: t("zmodem.manual.armFailedTitle"),
          description: String(error),
          variant: "destructive",
        })
      }
    },
    [sessionNonce, t, tabId]
  )

  const handleTerminalMenuAction = useCallback(
    async (action: string) => {
      if (action === "zmodem-send") {
        await armZmodemManualTrigger("send")
        return
      }
      if (action === "zmodem-receive") {
        await armZmodemManualTrigger("receive")
        return
      }
      const term = termRef.current
      const selection = terminalContextMenu?.selection.trim() ?? ""
      if (action === "clear-history") {
        clearTerminalHistory()
        return
      }
      if (action === "copy" && selection) {
        try {
          await invoke("plugin:clipboard-manager|write_text", { text: selection })
          term?.clearSelection()
        } catch (error) {
          console.error("Failed to copy terminal selection:", error)
          toast({
            title: t("terminalContext.copyFailedTitle"),
            description: t("terminalContext.copyFailedDescription"),
            variant: "destructive",
          })
        }
        return
      }
      if (action === "save" && selection) {
        const profile =
          connection?.profileId && connection.profileName
            ? { id: connection.profileId, name: connection.profileName }
            : undefined
        onSaveCommand?.(selection, profile)
        return
      }
      if (action === "find") {
        onOpenCommandLibrary?.(selection || undefined)
        return
      }
      if (action === "paste") {
        try {
          const clipboardText = await invoke<string>("plugin:clipboard-manager|read_text")
          if (clipboardText) term?.paste(clipboardText)
        } catch (error) {
          console.error("Failed to paste into terminal:", error)
          toast({
            title: t("terminalContext.pasteFailedTitle"),
            description: t("terminalContext.pasteFailedDescription"),
            variant: "destructive",
          })
        }
      }
    },
    [
      armZmodemManualTrigger,
      clearTerminalHistory,
      connection,
      onOpenCommandLibrary,
      onSaveCommand,
      t,
      terminalContextMenu,
    ]
  )

  const terminalMenuActions: TabContextMenuAction[] = [
    {
      action: "copy",
      label: t("terminalContext.copy"),
      icon: "copy",
      disabled: !terminalContextMenu?.selection,
    },
    {
      action: "save",
      label: t("terminalContext.saveCommand"),
      icon: "star",
      disabled: !terminalContextMenu?.selection,
    },
    { separator: true, action: "separator", label: "" },
    { action: "paste", label: t("terminalContext.paste"), icon: "paste" },
    { action: "find", label: t("terminalContext.findCommand"), icon: "search" },
    { separator: true, action: "separator", label: "" },
    { action: "zmodem-send", label: t("terminalContext.zmodemSend"), icon: "upload" },
    { action: "zmodem-receive", label: t("terminalContext.zmodemReceive"), icon: "download" },
    { separator: true, action: "separator", label: "" },
    {
      action: "clear-history",
      label: t("terminalContext.clearHistory"),
      icon: "x",
    },
  ]

  useEffect(() => {
    const unregisterFind = registerHandler("terminal.find", () => {
      if (!isActiveRef.current) return false
      openSearch()
    })
    const unregisterClear = registerHandler("terminal.clear", () => {
      if (!isActiveRef.current) return false
      clearTerminalHistory()
    })
    const unregisterToggleSftp = registerHandler("sftp.toggle", () => {
      if (!isActiveRef.current) return false
      handleToggleSftpDrawer()
    })
    const unregisterZmodemSend = registerHandler("zmodem.sendFiles", () => {
      if (!isActiveRef.current) return false
      void armZmodemManualTrigger("send")
    })
    const unregisterZmodemReceive = registerHandler("zmodem.receiveFiles", () => {
      if (!isActiveRef.current) return false
      void armZmodemManualTrigger("receive")
    })
    const unregisterFillSavedPassword = registerHandler("terminal.fillSavedPassword", () => {
      if (!isActiveRef.current) return false
      const actions = savedPasswordPromptActionsRef.current
      if (!actions) return false
      if (actions.fill()) {
        termRef.current?.focus()
        return
      }
      // Declining would let xterm send the combo as Enter, submitting a
      // half-typed password; at a password prompt the chord is a no-op.
      if (!actions.atPasswordPrompt()) return false
    })
    const unregisterSaveSelection = registerHandler("terminal.saveSelection", () => {
      if (!isActiveRef.current) return false
      if (!containerRef.current?.contains(document.activeElement)) return false
      const selection = termRef.current?.hasSelection() ? termRef.current.getSelection().trim() : ""
      if (!selection) return false
      const currentConnection = connectionRef.current
      const profile =
        currentConnection?.profileId && currentConnection.profileName
          ? { id: currentConnection.profileId, name: currentConnection.profileName }
          : undefined
      onSaveCommand?.(selection, profile)
    })
    return () => {
      unregisterFind()
      unregisterClear()
      unregisterToggleSftp()
      unregisterZmodemSend()
      unregisterZmodemReceive()
      unregisterSaveSelection()
      unregisterFillSavedPassword()
    }
  }, [
    armZmodemManualTrigger,
    clearTerminalHistory,
    connectionRef,
    handleToggleSftpDrawer,
    isActiveRef,
    onSaveCommand,
    openSearch,
    registerHandler,
  ])

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
      className={`terminal-tab-shell ${isActive ? "is-active" : ""} ${isBroadcastSource ? "is-broadcast-source" : ""}`}
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
            <Keyboard size={13} aria-hidden="true" />
            <strong>{t("broadcast.primaryInput")}</strong>
            <span aria-hidden="true">·</span>
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
          isGlobalShortcutTarget={isGlobalShortcutTarget}
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

        {terminalContextMenu && (
          <ContextMenu
            x={terminalContextMenu.x}
            y={terminalContextMenu.y}
            actions={terminalMenuActions}
            onAction={(action) => void handleTerminalMenuAction(action)}
            onClose={() => setTerminalContextMenu(null)}
          />
        )}

        {savedPasswordPrompt && (
          <SavedPasswordPromptBar
            connection={connection}
            fillShortcut={bindings["terminal.fillSavedPassword"]?.[0]}
            onDismiss={() => {
              savedPasswordPromptActionsRef.current?.dismiss()
              termRef.current?.focus()
            }}
            onFill={() => {
              savedPasswordPromptActionsRef.current?.fill()
              termRef.current?.focus()
            }}
            state={savedPasswordPrompt}
          />
        )}

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
