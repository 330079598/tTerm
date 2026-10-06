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
import type { CommandMarks } from "@/components/TerminalTab/commandMarks"
import { ConnectionHeader } from "@/components/TerminalTab/ConnectionHeader"
import { HostKeyPromptDialog } from "@/components/TerminalTab/HostKeyPromptDialog"
import { SshAuthPromptDialog } from "@/components/TerminalTab/SshAuthPromptDialog"
import { JumpHostInfoDialog } from "@/components/TerminalTab/JumpHostInfoDialog"
import { SavedPasswordPromptBar } from "@/components/TerminalTab/SavedPasswordPromptBar"
import { ServerMonitorBar } from "@/components/TerminalTab/ServerMonitorBar"
import { TerminalSearchBar } from "@/components/TerminalTab/TerminalSearchBar"
import { useTerminalSearch } from "@/components/TerminalTab/useTerminalSearch"
import { useTerminalLifecycle } from "@/components/TerminalTab/useTerminalLifecycle"
import { useZmodemTransfers } from "@/components/TerminalTab/useZmodemTransfers"
import {
  getConnectionDisplay,
  getShellIntegrationFallbackReason,
  TAB_ACTIVATE_REFIT_DELAY_MS,
} from "@/components/TerminalTab/terminalTabUtils"
import type {
  ConnectionState,
  HostKeyPromptState,
  SavedPasswordPromptActions,
  SavedPasswordPromptState,
  SshConnectionProgress,
  TerminalTabProps,
} from "@/components/TerminalTab/types"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { toast } from "@/hooks/use-toast"
import { isWindowBlurEnabled, useConfig } from "@/contexts/ConfigContext"
import { isTransparentTerminalTheme, withWindowBlur } from "@/lib/terminalPalette"
import type { TerminalRenderer } from "@/contexts/ConfigContext"
import { useKeymap } from "@/contexts/KeymapContext"
import { useCatalogFor, useTheme } from "@/contexts/ThemeContext"
import { useStableRef } from "@/hooks/useStableRef"
import { resolveScrollbackLines } from "@/lib/scrollback"
import { safePreloadFont, updateCanvasFontHostFont } from "@/lib/canvasFontHost"
import { pasteNeedsConfirmation, summarizePaste } from "@/lib/pasteGuard"
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
  onCwdChange,
  onReconnectRequest,
  onSavedPasswordPromptChange,
  onSessionUnavailable,
  onSensitivePrompt,
  onConnectionProgress,
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
  const onCwdChangeRef = useStableRef(onCwdChange)
  const onInputRef = useStableRef(onInput)
  const onCommandExecutedRef = useStableRef(onCommandExecuted)
  const onReconnectRequestRef = useStableRef(onReconnectRequest)
  const onSavedPasswordPromptChangeRef = useStableRef(onSavedPasswordPromptChange)
  const onSessionUnavailableRef = useStableRef(onSessionUnavailable)
  const onSensitivePromptRef = useStableRef(onSensitivePrompt)
  const onConnectionProgressRef = useStableRef(onConnectionProgress)
  const { config, saveConfig } = useConfig()
  const { displayedTheme, getTheme } = useTheme()
  const { t } = useTranslation()
  const configFontFamilyRef = useStableRef(config.font_family)
  const configFontSizeRef = useStableRef(config.font_size)
  const configLineHeightRef = useStableRef(config.terminal_line_height)
  const configLetterSpacingRef = useStableRef(config.terminal_letter_spacing)
  const configCursorStyleRef = useStableRef(config.cursor_style)
  const configMacOptionIsMetaRef = useStableRef(config.mac_option_is_meta)
  const configScrollbackLinesRef = useStableRef(config.scrollback_lines)
  const configTerminalRendererRef = useStableRef<TerminalRenderer>(config.terminal_renderer)
  const sessionResetKey = `${tabId}:${sessionNonce}:${connection?.type ?? "terminal"}`
  const defaultConnectionState: ConnectionState = "connecting"
  const savedPasswordPromptActionsRef = useRef<SavedPasswordPromptActions | null>(null)
  const commandMarksRef = useRef<CommandMarks | null>(null)
  const commandMarksEnabledRef = useStableRef(config.command_marks)
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
    hasCommandOutput: boolean
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

  const windowBlur = isWindowBlurEnabled(config)
  // A connection's own colors win over the app theme; one that no longer
  // exists falls back to the app theme.
  const connectionThemeId = connection?.type === "ssh" ? connection.terminalTheme : undefined
  // A library theme resolves once the library has loaded.
  useCatalogFor([connectionThemeId])
  const connectionPalette = connectionThemeId ? getTheme(connectionThemeId)?.terminal : undefined
  const resolveTerminalTheme = useCallback(
    () => ({
      ...withWindowBlur(
        connectionPalette ?? getTheme(displayedTheme)?.terminal ?? getTheme("default")!.terminal,
        windowBlur
      ),
      // The overview ruler under the scrollbar shows its marks only.
      overviewRulerBorder: "#00000000",
    }),
    [connectionPalette, displayedTheme, getTheme, windowBlur]
  )

  const terminalTheme = useMemo(() => resolveTerminalTheme(), [resolveTerminalTheme])
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
      // No PTY yet: create_pty sends the size itself, and catches up after.
      if (!prevSize) return
      if (!force && prevSize.rows === nextSize.rows && prevSize.cols === nextSize.cols) {
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
    commandMarksRef,
    commandMarksEnabledRef,
    connectionRef,
    containerRef,
    creatingPtyRef,
    fitAddonRef,
    fitTerminalOnly,
    initializedRef,
    configCursorStyleRef,
    configMacOptionIsMetaRef,
    configFontFamilyRef,
    configFontSizeRef,
    configLineHeightRef,
    configLetterSpacingRef,
    configScrollbackLinesRef,
    configTerminalRendererRef,
    terminalThemeRef,
    isActiveRef,
    lastPtySizeRef,
    onPidChangeRef,
    onCwdChangeRef,
    onInputRef,
    onCommandExecutedRef,
    onReconnectRequestRef,
    onSavedPasswordPromptChangeRef,
    onSessionUnavailableRef,
    onSensitivePromptRef,
    onConnectionProgressRef,
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

  const lastAppliedFontRef = useRef<{
    family: string
    size: number
    lineHeight: number
    letterSpacing: number
  } | null>(null)

  useEffect(() => {
    const term = termRef.current
    if (!term) return

    term.options.cursorStyle = config.cursor_style
    term.options.macOptionIsMeta = config.mac_option_is_meta

    const lastFont = lastAppliedFontRef.current
    const fontChanged =
      !lastFont ||
      lastFont.family !== config.font_family ||
      lastFont.size !== config.font_size ||
      lastFont.lineHeight !== config.terminal_line_height ||
      lastFont.letterSpacing !== config.terminal_letter_spacing

    if (fontChanged) {
      lastAppliedFontRef.current = {
        family: config.font_family,
        size: config.font_size,
        lineHeight: config.terminal_line_height,
        letterSpacing: config.terminal_letter_spacing,
      }
      updateCanvasFontHostFont(config.font_family, config.font_size)
      term.options.fontFamily = config.font_family
      term.options.fontSize = config.font_size
      term.options.lineHeight = config.terminal_line_height
      term.options.letterSpacing = config.terminal_letter_spacing

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
    config.mac_option_is_meta,
    config.font_family,
    config.font_size,
    config.terminal_letter_spacing,
    config.terminal_line_height,
    isActiveRef,
    scheduleFitDuringResize,
  ])

  useEffect(() => {
    commandMarksRef.current?.setEnabled(config.command_marks)
  }, [config.command_marks, sessionNonce])

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

    const theme = resolveTerminalTheme()
    term.options.allowTransparency = isTransparentTerminalTheme(theme)
    term.options.theme = theme
  }, [resolveTerminalTheme, sessionNonce])

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

  // Once per tab: reconnects that fall back again for the same reason stay quiet.
  const shellIntegrationNoticeShownRef = useRef(false)
  const shellIntegration = connectionProgress?.shellIntegration
  useEffect(() => {
    if (shellIntegration?.status !== "unavailable" || shellIntegrationNoticeShownRef.current) {
      return
    }
    shellIntegrationNoticeShownRef.current = true
    toast({
      title: t("sessionHeader.shellIntegrationFallback", {
        connection: getConnectionDisplay(connectionRef.current, t),
      }),
      description: getShellIntegrationFallbackReason(shellIntegration.reason, t),
    })
  }, [connectionRef, shellIntegration, t])

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

  const { confirm: confirmPaste, ConfirmDialog: PasteConfirmDialog } = useConfirmDialog()
  const confirmMultilinePasteRef = useStableRef(config.confirm_multiline_paste)
  // A live broadcast source asks on its own before a multiline paste goes out.
  const broadcastConfirmsPasteRef = useStableRef(
    isBroadcastSource && liveBroadcastState === "active"
  )

  const pasteNeedsConfirm = useCallback(
    (term: Terminal, text: string) =>
      !broadcastConfirmsPasteRef.current &&
      pasteNeedsConfirmation(text, {
        enabled: confirmMultilinePasteRef.current,
        bracketedPasteMode: term.modes.bracketedPasteMode,
      }),
    [broadcastConfirmsPasteRef, confirmMultilinePasteRef]
  )

  /** Pastes into the terminal, first asking when a line break would run a command. */
  const pasteIntoTerminal = useCallback(
    async (text: string) => {
      const term = termRef.current
      if (!term || !text) return
      if (pasteNeedsConfirm(term, text)) {
        const { lineCount, preview } = summarizePaste(text)
        const confirmed = await confirmPaste({
          title: t("terminalPaste.multilineTitle"),
          description: (
            <>
              {lineCount > 1
                ? t("terminalPaste.multilineDescription", { count: lineCount })
                : t("terminalPaste.trailingLineBreakDescription")}
              <span className="bg-muted text-foreground mt-3 block max-h-48 overflow-auto rounded-md border px-2 py-1.5 font-mono text-xs whitespace-pre">
                {preview}
              </span>
            </>
          ),
          confirmText: t("terminalPaste.pasteAnyway"),
          defaultAction: "confirm",
        })
        // The terminal may have been recreated while the dialog was open.
        if (termRef.current !== term) return
        term.focus()
        if (!confirmed) return
      }
      term.paste(text)
    },
    [confirmPaste, pasteNeedsConfirm, t]
  )

  // Keyboard and menu-bar pastes reach xterm as DOM paste events; hold back
  // the ones that need confirmation before xterm sends them.
  useEffect(() => {
    const handlePaste = (event: ClipboardEvent) => {
      const container = containerRef.current
      const term = termRef.current
      if (!container || !term) return
      if (!(event.target instanceof Node) || !container.contains(event.target)) return
      const text = event.clipboardData?.getData("text/plain") ?? ""
      if (!pasteNeedsConfirm(term, text)) return
      event.preventDefault()
      event.stopPropagation()
      void pasteIntoTerminal(text)
    }
    document.addEventListener("paste", handlePaste, true)
    return () => document.removeEventListener("paste", handlePaste, true)
  }, [pasteIntoTerminal, pasteNeedsConfirm])

  const pasteFromClipboard = useCallback(async () => {
    try {
      const clipboardText = await invoke<string>("plugin:clipboard-manager|read_text")
      if (clipboardText) await pasteIntoTerminal(clipboardText)
    } catch (error) {
      console.error("Failed to paste into terminal:", error)
      toast({
        title: t("terminalContext.pasteFailedTitle"),
        description: t("terminalContext.pasteFailedDescription"),
        variant: "destructive",
      })
    }
  }, [pasteIntoTerminal, t])

  // Copy on select: the selection is final once the mouse button that made
  // it is released, which may happen outside the terminal.
  useEffect(() => {
    const container = containerRef.current
    if (!config.copy_on_select || !container) return

    let copyTimer: number | null = null
    const copySelection = () => {
      copyTimer = window.setTimeout(() => {
        copyTimer = null
        const term = termRef.current
        const selection = term?.hasSelection() ? term.getSelection() : ""
        if (!selection) return
        invoke("plugin:clipboard-manager|write_text", { text: selection }).catch((error) => {
          console.error("Failed to copy terminal selection:", error)
        })
      }, 0)
    }
    const handleMouseDown = (event: MouseEvent) => {
      if (event.button !== 0) return
      document.removeEventListener("mouseup", copySelection, true)
      document.addEventListener("mouseup", copySelection, { capture: true, once: true })
    }

    container.addEventListener("mousedown", handleMouseDown, true)
    return () => {
      container.removeEventListener("mousedown", handleMouseDown, true)
      document.removeEventListener("mouseup", copySelection, true)
      if (copyTimer !== null) window.clearTimeout(copyTimer)
    }
  }, [config.copy_on_select])

  const handleTerminalContextMenu = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      event.preventDefault()
      const term = termRef.current
      term?.focus()
      if (showSftpDrawer) setShowSftpDrawer(false)
      // Shift+right-click still opens the menu.
      if (config.right_click_paste && !event.shiftKey) {
        void pasteFromClipboard()
        return
      }
      setTerminalContextMenu({
        x: event.clientX,
        y: event.clientY,
        selection: term?.hasSelection() ? term.getSelection() : "",
        hasCommandOutput: commandMarksRef.current?.lastOutputRange() != null,
      })
    },
    [config.right_click_paste, pasteFromClipboard, showSftpDrawer]
  )

  const clearTerminalHistory = useCallback(() => {
    const term = termRef.current
    if (!term) return

    term.clear()
    term.scrollToBottom()
    term.clearSelection()
    term.focus()
  }, [])

  /** Selects and copies the output of the last command the shell marked. */
  const copyLastCommandOutput = useCallback(async () => {
    const marks = commandMarksRef.current
    const text = marks?.lastOutputText()
    if (!marks || text == null) return false
    if (!text) {
      toast({ title: t("terminalContext.commandOutputEmpty") })
      return true
    }
    marks.selectLastOutput()
    try {
      await invoke("plugin:clipboard-manager|write_text", { text })
    } catch (error) {
      console.error("Failed to copy command output:", error)
      toast({
        title: t("terminalContext.copyFailedTitle"),
        description: t("terminalContext.copyFailedDescription"),
        variant: "destructive",
      })
    }
    return true
  }, [t])

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
      if (action === "copy-command-output") {
        await copyLastCommandOutput()
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
        await pasteFromClipboard()
      }
    },
    [
      armZmodemManualTrigger,
      clearTerminalHistory,
      connection,
      copyLastCommandOutput,
      onOpenCommandLibrary,
      onSaveCommand,
      pasteFromClipboard,
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
      action: "copy-command-output",
      label: t("terminalContext.copyCommandOutput"),
      icon: "copy",
      disabled: !terminalContextMenu?.hasCommandOutput,
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
    const unregisterPreviousCommand = registerHandler("terminal.previousCommand", () => {
      if (!isActiveRef.current) return false
      return commandMarksRef.current?.scrollToPreviousPrompt() ?? false
    })
    const unregisterNextCommand = registerHandler("terminal.nextCommand", () => {
      if (!isActiveRef.current) return false
      return commandMarksRef.current?.scrollToNextPrompt() ?? false
    })
    const unregisterCopyCommandOutput = registerHandler("terminal.copyLastCommandOutput", () => {
      if (!isActiveRef.current || !commandMarksRef.current?.lastOutputRange()) return false
      void copyLastCommandOutput()
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
      unregisterPreviousCommand()
      unregisterNextCommand()
      unregisterCopyCommandOutput()
    }
  }, [
    armZmodemManualTrigger,
    clearTerminalHistory,
    connectionRef,
    copyLastCommandOutput,
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
            <span>
              {liveBroadcastState === "paused"
                ? t("broadcast.livePaused")
                : t("broadcast.liveActive")}
            </span>
          </span>
          <div>
            {liveBroadcastState === "paused" ? (
              <button type="button" onClick={onResumeBroadcast}>
                <Play size={13} aria-hidden="true" />
                <span>{t("broadcast.resumeLive")}</span>
              </button>
            ) : (
              <button type="button" onClick={onPauseBroadcast}>
                <Pause size={13} aria-hidden="true" />
                <span>{t("broadcast.pauseLive")}</span>
              </button>
            )}
            <button type="button" className="destructive" onClick={onStopBroadcast}>
              <Square size={13} fill="currentColor" aria-hidden="true" />
              <span>{t("broadcast.stopLive")}</span>
            </button>
          </div>
        </div>
      )}

      <div
        ref={surfaceRef}
        className="terminal-surface"
        style={{
          ...terminalPaddingStyle,
          // The padding around the grid takes the connection's background too.
          ...(connectionPalette && !windowBlur ? { background: connectionPalette.background } : {}),
        }}
      >
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
          className="terminal-host"
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
            backgroundColor: windowBlur
              ? "transparent"
              : (connectionPalette?.background ?? "hsl(var(--background))"),
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
        <SshAuthPromptDialog
          tabId={tabId}
          visible={isActive}
          remembersPasswords={connection?.authMethod === "auto"}
        />
        <PasteConfirmDialog />
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
