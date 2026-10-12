import { useEffect, useRef } from "react"
import { useTranslation } from "react-i18next"
import type { IDisposable } from "@xterm/xterm"
import { invoke } from "@tauri-apps/api/core"

import type {
  TerminalSessionContext,
  TerminalSessionState,
  UseTerminalLifecycleOptions,
} from "@/components/TerminalTab/terminalLifecycleTypes"
import {
  useTerminalRenderer,
  useTerminalRendererSync,
} from "@/components/TerminalTab/useTerminalRenderer"
import { openTerminal } from "@/components/TerminalTab/openTerminal"
import { createConnectionStateUpdater } from "@/components/TerminalTab/connectionStateUpdater"
import { createSavedPasswordPrompts } from "@/components/TerminalTab/savedPasswordPrompts"
import { createCommandTracking } from "@/components/TerminalTab/commandTracking"
import { attachTerminalInput } from "@/components/TerminalTab/terminalInput"
import { startPtySession } from "@/components/TerminalTab/ptySession"
import { disposeRendererAddon } from "@/components/TerminalTab/rendererRelease"
import { safePreloadFont } from "@/lib/canvasFontHost"
import { EMPTY_COMMAND_CAPTURE_STATE } from "@/lib/terminalCommandCapture"

export function useTerminalLifecycle({
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
  initialCursorStyle,
  initialFontFamily,
  initialFontSize,
  initialScrollbackLines,
  initialTerminalRenderer,
  initialTerminalThemeRef,
  hiddenRendererReleaseSecs,
  isActive,
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
  onAttentionRef,
  onBackgroundOutputRef,
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
  terminalRenderer,
  termRef,
  waitingForReconnectRef,
}: UseTerminalLifecycleOptions) {
  const {
    activeRendererAddonRef,
    lastRendererRef,
    rendererSuspendedRef,
    webglRetryTimerRef,
    loadTerminalRenderer,
    clearTextureAtlas,
  } = useTerminalRenderer(termRef)

  // Kept in a ref so event-time terminal banners translate with the active
  // language without re-running the terminal-creation effect.
  const { t } = useTranslation()
  const translationRef = useRef(t)
  useEffect(() => {
    translationRef.current = t
  }, [t])

  const cursorStyleRef = (configCursorStyleRef ?? initialCursorStyle)!
  const fontFamilyRef = (configFontFamilyRef ?? initialFontFamily)!
  const fontSizeRef = (configFontSizeRef ?? initialFontSize)!
  const scrollbackLinesRef = (configScrollbackLinesRef ?? initialScrollbackLines)!
  const rendererRef = (configTerminalRendererRef ?? initialTerminalRenderer)!
  const themeRef = (terminalThemeRef ?? initialTerminalThemeRef)!

  useEffect(() => {
    const container = containerRef.current
    if (!container || initializedRef.current) return
    const onSavedPasswordPromptChange = onSavedPasswordPromptChangeRef.current
    initializedRef.current = true
    waitingForReconnectRef.current = false
    setSavedPasswordPrompt(null)

    const { term, imeCursorGuard, imeEarlyInputFix, scrollbackMemory, imeFocusRepair } =
      openTerminal({
        container,
        cursorStyleRef,
        configMacOptionIsMetaRef,
        scrollbackLinesRef,
        fontSizeRef,
        fontFamilyRef,
        configLetterSpacingRef,
        configLineHeightRef,
        themeRef,
        termRef,
        fitAddonRef,
        searchAddonRef,
        searchResultsDisposableRef,
        setSearchResults,
      })

    // A tab restored in the background draws nothing until it is shown.
    if (isActiveRef.current) {
      loadTerminalRenderer(terminalRenderer ?? rendererRef.current, term)
    } else {
      rendererSuspendedRef.current = true
    }

    const updateScrollbackState = () => {
      container.classList.toggle("xterm-has-scrollback", term.buffer.active.baseY > 0)
    }
    const scrollbackDisposables: IDisposable[] = [
      term.onWriteParsed(updateScrollbackState),
      term.onResize(updateScrollbackState),
    ]
    updateScrollbackState()

    if (isActiveRef.current) {
      term.focus()
    }
    fitTerminalOnly()

    const session: TerminalSessionState = {
      disposed: false,
      passwordPromptCheckId: 0,
      commandCaptureState: EMPTY_COMMAND_CAPTURE_STATE,
      commandCaptureSuspended: false,
      lineFeedCount: 0,
      currentPromptKey: null,
      lastPasswordFill: null,
      loginScriptRunner: null,
    }

    const handleFontsLoaded = () => {
      if (session.disposed) return
      fitTerminalOnly()
      term.refresh(0, Math.max(0, term.rows - 1))
    }

    void document.fonts?.ready?.then(handleFontsLoaded)
    document.fonts?.addEventListener?.("loadingdone", handleFontsLoaded)

    void safePreloadFont(fontSizeRef.current, fontFamilyRef.current).then((loaded) => {
      if (loaded && !session.disposed) {
        handleFontsLoaded()
      }
    })

    const ctx: TerminalSessionContext = {
      term,
      session,
      tabId,
      sessionNonce,
      commandMarksEnabledRef,
      commandMarksRef,
      connectionRef,
      creatingPtyRef,
      fitTerminalOnly,
      isActiveRef,
      lastPtySizeRef,
      onAttentionRef,
      onBackgroundOutputRef,
      onCommandExecutedRef,
      onConnectionProgressRef,
      onCwdChangeRef,
      onInputRef,
      onPidChangeRef,
      onReconnectRequestRef,
      onSensitivePromptRef,
      onSessionUnavailableRef,
      savedPasswordPromptActionsRef,
      setConnectionProgress,
      setHostKeyPrompt,
      setSavedPasswordPrompt,
      sudoPromptPatternsRef,
      waitingForReconnectRef,
      onSavedPasswordPromptChange,
      translationRef,
      scrollbackMemory,
    }
    const { applyConnectionState, setConnectionStateIfChanged } =
      createConnectionStateUpdater(setConnectionState)
    const { answerCurrentPrompt, checkPasswordPrompt, stopLoginScript, startLoginScript } =
      createSavedPasswordPrompts(ctx)
    const {
      agentTracker,
      commandTimer,
      emitExecutedCommand,
      commandMarks,
      shellIntegrationDisposables,
      attentionReports,
      cwdReports,
      lineFeedDisposable,
    } = createCommandTracking(ctx)
    attachTerminalInput(ctx, {
      applyConnectionState,
      agentTracker,
      stopLoginScript,
      answerCurrentPrompt,
      commandMarks,
      commandTimer,
      emitExecutedCommand,
    })
    const { outputAcker, stopListening } = startPtySession(ctx, {
      applyConnectionState,
      setConnectionStateIfChanged,
      checkPasswordPrompt,
      stopLoginScript,
      startLoginScript,
      agentTracker,
      commandTimer,
    })

    const resizeObserver = new ResizeObserver(() => {
      if (!isActiveRef.current) return
      scheduleFitDuringResize()
    })
    resizeObserverRef.current = resizeObserver

    if (isActiveRef.current) {
      resizeObserver.observe(container)
      if (surfaceRef.current) {
        resizeObserver.observe(surfaceRef.current)
      }
    }

    return () => {
      session.disposed = true
      outputAcker.dispose()
      session.passwordPromptCheckId += 1
      stopLoginScript()
      savedPasswordPromptActionsRef.current = null
      lineFeedDisposable.dispose()

      resizeObserver.disconnect()
      resizeObserverRef.current = null

      if (resizeRafRef.current !== null) {
        window.cancelAnimationFrame(resizeRafRef.current)
        resizeRafRef.current = null
      }

      if (resizePtySyncTimerRef.current !== null) {
        window.clearTimeout(resizePtySyncTimerRef.current)
        resizePtySyncTimerRef.current = null
      }

      if (activateFitTimerRef.current !== null) {
        window.clearTimeout(activateFitTimerRef.current)
        activateFitTimerRef.current = null
      }

      stopListening()
      invoke("kill_pty", { tabId, sessionNonce }).catch(console.error)
      searchResultsDisposableRef.current?.dispose()
      searchResultsDisposableRef.current = null
      searchAddonRef.current = null
      if (typeof document !== "undefined") {
        document.fonts?.removeEventListener?.("loadingdone", handleFontsLoaded)
      }
      if (activeRendererAddonRef.current) {
        try {
          disposeRendererAddon(term, activeRendererAddonRef.current)
        } catch (disposeErr) {
          console.warn("Failed to dispose active renderer addon during unmount:", disposeErr)
        }
        activeRendererAddonRef.current = null
      }
      lastRendererRef.current = null
      rendererSuspendedRef.current = false
      if (webglRetryTimerRef.current !== null) {
        window.clearTimeout(webglRetryTimerRef.current)
        webglRetryTimerRef.current = null
      }
      imeCursorGuard.dispose()
      imeEarlyInputFix.dispose()
      imeFocusRepair?.dispose()
      scrollbackMemory.dispose()
      term.dispose()
      termRef.current = null
      fitAddonRef.current = null
      initializedRef.current = false
      lastPtySizeRef.current = null
      creatingPtyRef.current = false
      waitingForReconnectRef.current = false
      setSavedPasswordPrompt(null)
      onSavedPasswordPromptChange?.(tabId, sessionNonce, null)
      for (const disposable of scrollbackDisposables) disposable.dispose()
      for (const disposable of shellIntegrationDisposables) disposable.dispose()
      attentionReports.dispose()
      agentTracker.reset()
      commandTimer.prompt()
      commandMarks.dispose()
      commandMarksRef.current = null
      cwdReports.dispose()
      container.classList.remove("xterm-has-scrollback")
      container.replaceChildren()
    }
  }, [
    activateFitTimerRef,
    commandMarksEnabledRef,
    commandMarksRef,
    connectionRef,
    containerRef,
    configLetterSpacingRef,
    configLineHeightRef,
    configMacOptionIsMetaRef,
    creatingPtyRef,
    cursorStyleRef,
    fitAddonRef,
    fitTerminalOnly,
    fontFamilyRef,
    fontSizeRef,
    initializedRef,
    isActiveRef,
    lastPtySizeRef,
    loadTerminalRenderer,
    onPidChangeRef,
    onCwdChangeRef,
    onInputRef,
    onCommandExecutedRef,
    onReconnectRequestRef,
    onSavedPasswordPromptChangeRef,
    onSessionUnavailableRef,
    onSensitivePromptRef,
    onAttentionRef,
    onBackgroundOutputRef,
    onConnectionProgressRef,
    savedPasswordPromptActionsRef,
    setSavedPasswordPrompt,
    sudoPromptPatternsRef,
    rendererRef,
    resizeObserverRef,
    resizePtySyncTimerRef,
    resizeRafRef,
    scheduleFitDuringResize,
    scrollbackLinesRef,
    surfaceRef,
    searchAddonRef,
    searchResultsDisposableRef,
    setConnectionState,
    setHostKeyPrompt,
    setConnectionProgress,
    setSearchResults,
    sessionNonce,
    tabId,
    terminalRenderer,
    termRef,
    themeRef,
    waitingForReconnectRef,
    rendererSuspendedRef,
    activeRendererAddonRef,
    lastRendererRef,
    webglRetryTimerRef,
  ])

  useTerminalRendererSync({
    termRef,
    initializedRef,
    terminalRenderer,
    isActiveRef,
    fitTerminalOnly,
    isActive,
    hiddenRendererReleaseSecs,
    sessionNonce,
    activeRendererAddonRef,
    lastRendererRef,
    rendererSuspendedRef,
    loadTerminalRenderer,
    rendererRef,
  })

  return {
    clearTextureAtlas,
  }
}
