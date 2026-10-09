import { useCallback, useEffect, useLayoutEffect, useRef } from "react"
import { useTranslation } from "react-i18next"
import { CanvasAddon } from "@xterm/addon-canvas"
import { FitAddon } from "@xterm/addon-fit"
import { SearchAddon, type ISearchResultChangeEvent } from "@xterm/addon-search"
import { Unicode11Addon } from "@xterm/addon-unicode11"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { WebglAddon } from "@xterm/addon-webgl"
import { type IDisposable, Terminal } from "@xterm/xterm"
import { Channel, invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { openUrl } from "@tauri-apps/plugin-opener"
import { platform } from "@tauri-apps/plugin-os"

import {
  CommandMarks,
  COMMAND_MARK_OSC_CODES,
  parseCommandMark,
} from "@/components/TerminalTab/commandMarks"
import { installImeCursorGuard } from "@/components/TerminalTab/imeCursorGuard"
import { installImeEarlyInputFix } from "@/components/TerminalTab/imeEarlyInput"
import { installImeFocusRepair } from "@/components/TerminalTab/imeFocusRepair"
import { OutputAcker } from "@/components/TerminalTab/outputAck"
import { disposeRendererAddon, renderAllRowsNow } from "@/components/TerminalTab/rendererRelease"
import { createScrollbackMemory } from "@/components/TerminalTab/scrollbackMemory"
import { getConnectionDisplay } from "@/components/TerminalTab/terminalTabUtils"
import type {
  ConnectionState,
  HostKeyPromptState,
  SavedPasswordPromptActions,
  SavedPasswordPromptState,
  SshConnectionProgress,
  SudoPasswordSource,
  TerminalTabProps,
} from "@/components/TerminalTab/types"
import { notifySavedPasswordNotSent } from "@/components/TerminalTab/savedPasswordNotice"
import { startingScrollbackLines } from "@/lib/scrollback"
import { isTransparentTerminalTheme } from "@/lib/terminalPalette"
import type { TerminalRenderer } from "@/contexts/ConfigContext"
import { safePreloadFont, updateCanvasFontHostFont } from "@/lib/canvasFontHost"
import {
  decodeOutputChunk,
  EMPTY_OUTPUT_SCAN_STATE,
  scanTerminalOutput,
  type TerminalOutputScanState,
} from "@/lib/terminalOutputScanner"
import { matchPasswordPrompt, readCursorLine, type PasswordPromptMatch } from "@/lib/sudoPrompt"
import { LoginScriptRunner, parseLoginScript } from "@/lib/loginScript"
import { CWD_REPORT_OSC_CODES, parseCwdReport } from "@/lib/terminalCwd"
import { AgentTracker, parseAgentReport } from "@/lib/agentStatus"
import { setTabAgentStatus } from "@/lib/tabAttention"
import {
  CommandTimer,
  KittyNotificationAssembler,
  NOTIFICATION_OSC_CODES,
  parseOsc777Notification,
  parseOsc9Notification,
  type TerminalAttentionEvent,
} from "@/lib/terminalNotifications"
import {
  captureTerminalInput,
  EMPTY_COMMAND_CAPTURE_STATE,
  parseShellIntegrationCommand,
} from "@/lib/terminalCommandCapture"

type ActiveRendererAddon = (WebglAddon | CanvasAddon) & {
  dispose: () => void
  clearTextureAtlas?: () => void
}

type UseTerminalLifecycleOptions = {
  activateFitTimerRef: React.RefObject<number | null>
  commandMarksRef: React.RefObject<CommandMarks | null>
  commandMarksEnabledRef: React.RefObject<boolean>
  connectionRef: React.RefObject<TerminalTabProps["connection"]>
  containerRef: React.RefObject<HTMLDivElement | null>
  creatingPtyRef: React.RefObject<boolean>
  fitAddonRef: React.RefObject<FitAddon | null>
  fitTerminalOnly: () => boolean
  initializedRef: React.RefObject<boolean>
  configCursorStyleRef?: React.RefObject<Terminal["options"]["cursorStyle"]>
  /** macOS: Option+key sends Meta instead of the alternate character. */
  configMacOptionIsMetaRef?: React.RefObject<boolean>
  configFontFamilyRef?: React.RefObject<string>
  configFontSizeRef?: React.RefObject<number>
  configLineHeightRef?: React.RefObject<number>
  configLetterSpacingRef?: React.RefObject<number>
  configScrollbackLinesRef?: React.RefObject<number>
  configTerminalRendererRef?: React.RefObject<TerminalRenderer>
  terminalThemeRef?: React.RefObject<NonNullable<Terminal["options"]["theme"]>>
  initialCursorStyle?: React.RefObject<Terminal["options"]["cursorStyle"]>
  initialFontFamily?: React.RefObject<string>
  initialFontSize?: React.RefObject<number>
  initialScrollbackLines?: React.RefObject<number>
  initialTerminalRenderer?: React.RefObject<TerminalRenderer>
  initialTerminalThemeRef?: React.RefObject<NonNullable<Terminal["options"]["theme"]>>
  terminalRenderer?: TerminalRenderer
  /**
   * Seconds a terminal stays out of sight before its renderer is released, so
   * switching back and forth between tabs keeps it and a tab left in the
   * background gives back its canvases; 0 keeps it.
   */
  hiddenRendererReleaseSecs: number
  isActive: boolean
  isActiveRef: React.RefObject<boolean>
  lastPtySizeRef: React.RefObject<{ rows: number; cols: number } | null>
  onPidChangeRef: React.RefObject<TerminalTabProps["onPidChange"]>
  onCwdChangeRef: React.RefObject<TerminalTabProps["onCwdChange"]>
  onInputRef: React.RefObject<TerminalTabProps["onInput"]>
  onCommandExecutedRef: React.RefObject<TerminalTabProps["onCommandExecuted"]>
  onReconnectRequestRef: React.RefObject<TerminalTabProps["onReconnectRequest"]>
  onSavedPasswordPromptChangeRef: React.RefObject<TerminalTabProps["onSavedPasswordPromptChange"]>
  onSessionUnavailableRef: React.RefObject<TerminalTabProps["onSessionUnavailable"]>
  onSensitivePromptRef: React.RefObject<TerminalTabProps["onSensitivePrompt"]>
  onConnectionProgressRef: React.RefObject<TerminalTabProps["onConnectionProgress"]>
  /** A finished command, a notification request, an agent's report or a bell. */
  onAttentionRef: React.RefObject<(event: TerminalAttentionEvent) => void>
  /** Output arrived while the tab was out of sight. */
  onBackgroundOutputRef: React.RefObject<() => void>
  savedPasswordPromptActionsRef: React.RefObject<SavedPasswordPromptActions | null>
  setSavedPasswordPrompt: (value: SavedPasswordPromptState | null) => void
  sudoPromptPatternsRef: React.RefObject<readonly RegExp[]>
  resizeObserverRef: React.RefObject<ResizeObserver | null>
  resizePtySyncTimerRef: React.RefObject<number | null>
  resizeRafRef: React.RefObject<number | null>
  scheduleFitDuringResize: () => void
  surfaceRef: React.RefObject<HTMLDivElement | null>
  searchAddonRef: React.RefObject<SearchAddon | null>
  searchResultsDisposableRef: React.RefObject<IDisposable | null>
  setConnectionState: (value: ConnectionState) => void
  setHostKeyPrompt: (value: HostKeyPromptState | null) => void
  setConnectionProgress: (value: SshConnectionProgress | null) => void
  setSearchResults: React.Dispatch<React.SetStateAction<ISearchResultChangeEvent>>
  sessionNonce: number
  tabId: string
  termRef: React.RefObject<Terminal | null>
  waitingForReconnectRef: React.RefObject<boolean>
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

// The IME focus loss it repairs is specific to WebView2.
const IS_WINDOWS = (() => {
  try {
    return platform() === "windows"
  } catch {
    return typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent)
  }
})()

function isLinkOpenModifierPressed(event: MouseEvent) {
  return LINK_MODIFIER_IS_CMD ? event.metaKey : event.ctrlKey
}

/**
 * The same prompt returning this soon after a fill means the password was
 * refused; sudo's failure delay is about two seconds.
 */
const PASSWORD_REJECT_WINDOW_MS = 20_000
/** Wait after a lost WebGL context before trying WebGL again. */
const WEBGL_RETRY_DELAY_MS = 30_000
/** WebGL retries per terminal after lost contexts. */
const MAX_WEBGL_RETRIES = 3

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
  const activeRendererAddonRef = useRef<ActiveRendererAddon | null>(null)
  const lastRendererRef = useRef<TerminalRenderer | null>(null)
  // Out of sight long enough to have its renderer released; showing the
  // terminal again loads it back.
  const rendererSuspendedRef = useRef(false)
  const webglRetriesRef = useRef(0)
  const webglRetryTimerRef = useRef<number | null>(null)

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

  const loadTerminalRenderer = useCallback(
    (targetRenderer: TerminalRenderer, targetTerm?: Terminal) => {
      const term = targetTerm ?? termRef.current
      if (!term) return

      if (activeRendererAddonRef.current) {
        try {
          disposeRendererAddon(term, activeRendererAddonRef.current)
        } catch (error) {
          console.error("Failed to dispose active terminal renderer addon:", error)
        }
        activeRendererAddonRef.current = null
      }

      if (targetRenderer === "webgl") {
        const loadWebgl = (): boolean => {
          try {
            const webglAddon = new WebglAddon()
            webglAddon.onContextLoss(() => {
              console.warn("WebGL context lost; falling back to canvas renderer")
              try {
                webglAddon.dispose()
              } catch (disposeErr) {
                console.warn("Failed to dispose WebGL addon on context loss:", disposeErr)
              }
              if (activeRendererAddonRef.current === webglAddon) {
                activeRendererAddonRef.current = null
              }
              let canvasAddon: CanvasAddon
              try {
                canvasAddon = new CanvasAddon()
                term.loadAddon(canvasAddon)
                activeRendererAddonRef.current = canvasAddon
                lastRendererRef.current = "canvas"
              } catch (canvasErr) {
                console.error("Failed to load canvas fallback after WebGL context loss:", canvasErr)
                return
              }
              // The canvas fallback keeps four full-size layers, so go back to
              // WebGL once the GPU may have recovered (a GPU switch or reset).
              // A terminal released while hidden gets WebGL back when shown;
              // the retries are capped so a page over WebKit's context limit
              // does not keep evicting other terminals' contexts.
              if (webglRetriesRef.current >= MAX_WEBGL_RETRIES) return
              webglRetriesRef.current += 1
              webglRetryTimerRef.current = window.setTimeout(() => {
                webglRetryTimerRef.current = null
                if (activeRendererAddonRef.current !== canvasAddon) return
                try {
                  disposeRendererAddon(term, canvasAddon)
                } catch (disposeErr) {
                  console.warn("Failed to dispose canvas fallback before WebGL retry:", disposeErr)
                }
                activeRendererAddonRef.current = null
                lastRendererRef.current = null
                if (!loadWebgl()) {
                  try {
                    const fallback = new CanvasAddon()
                    term.loadAddon(fallback)
                    activeRendererAddonRef.current = fallback
                    lastRendererRef.current = "canvas"
                  } catch (canvasErr) {
                    console.error("Failed to reload canvas renderer after WebGL retry:", canvasErr)
                  }
                }
                renderAllRowsNow(term)
              }, WEBGL_RETRY_DELAY_MS)
            })
            term.loadAddon(webglAddon)
            activeRendererAddonRef.current = webglAddon
            lastRendererRef.current = "webgl"
            return true
          } catch (error) {
            console.warn(
              "WebGL not supported in this environment; falling back to canvas renderer",
              error
            )
            return false
          }
        }
        if (loadWebgl()) return
      }

      // "canvas" mode or fallback from failed WebGL
      try {
        const canvasAddon = new CanvasAddon()
        term.loadAddon(canvasAddon)
        activeRendererAddonRef.current = canvasAddon
        lastRendererRef.current = "canvas"
      } catch (error) {
        console.error("Failed to load canvas renderer; using DOM renderer fallback", error)
        lastRendererRef.current = null
      }
    },
    [termRef]
  )

  useEffect(() => {
    const container = containerRef.current
    if (!container || initializedRef.current) return
    const onSavedPasswordPromptChange = onSavedPasswordPromptChangeRef.current
    initializedRef.current = true
    waitingForReconnectRef.current = false
    setSavedPasswordPrompt(null)

    // Ensure WebKit resolves the local font on the canvas font host and container before warmUp
    updateCanvasFontHostFont(fontFamilyRef.current, fontSizeRef.current)
    container.style.fontFamily = fontFamilyRef.current
    try {
      void container.offsetWidth
    } catch {
      // Ignore in non-browser environments
    }

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: cursorStyleRef.current,
      macOptionIsMeta: configMacOptionIsMetaRef?.current ?? false,
      scrollback: startingScrollbackLines(scrollbackLinesRef.current),
      fontSize: fontSizeRef.current,
      fontFamily: fontFamilyRef.current,
      fontWeight: "normal",
      fontWeightBold: "bold",
      letterSpacing: configLetterSpacingRef?.current ?? 0,
      lineHeight: configLineHeightRef?.current ?? 1,
      theme: themeRef.current,
      allowTransparency: isTransparentTerminalTheme(themeRef.current),
      allowProposedApi: true,
      // Under the 10px scrollbar (TerminalTab.css): search matches, commands.
      overviewRuler: { width: 10 },
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

    termRef.current = term
    fitAddonRef.current = fitAddon
    searchAddonRef.current = searchAddon
    searchResultsDisposableRef.current = searchAddon.onDidChangeResults((results) => {
      setSearchResults(results)
    })

    container.replaceChildren()
    term.open(container)
    const imeCursorGuard = installImeCursorGuard(term)
    const imeEarlyInputFix = installImeEarlyInputFix(term)
    const scrollbackMemory = createScrollbackMemory(term, () => scrollbackLinesRef.current)
    const imeFocusRepair = IS_WINDOWS
      ? installImeFocusRepair(term, () => invoke("repair_ime_focus"))
      : null

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

    let disposed = false

    let createdPtySize: { rows: number; cols: number } | null = null
    const waitForStableFit = async () => {
      let stableFrames = 0
      for (let frame = 0; frame < 60 && stableFrames < 4; frame += 1) {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
        if (disposed) return
        if (fitTerminalOnly()) stableFrames += 1
        else stableFrames = 0
      }
    }

    const handleFontsLoaded = () => {
      if (disposed) return
      fitTerminalOnly()
      term.refresh(0, Math.max(0, term.rows - 1))
    }

    void document.fonts?.ready?.then(handleFontsLoaded)
    document.fonts?.addEventListener?.("loadingdone", handleFontsLoaded)

    void safePreloadFont(fontSizeRef.current, fontFamilyRef.current).then((loaded) => {
      if (loaded && !disposed) {
        handleFontsLoaded()
      }
    })
    let passwordPromptCheckId = 0
    let commandCaptureState = EMPTY_COMMAND_CAPTURE_STATE
    let commandCaptureSuspended = false
    let lastEmittedCommand: { text: string; at: number } | null = null
    let outputScanState: TerminalOutputScanState = EMPTY_OUTPUT_SCAN_STATE
    let lastConnectionState: ConnectionState = "connecting"
    // Line feeds seen so far: a prompt printed again after a newline (sudo
    // retrying) gets a new key even when it lands on the same screen row.
    let lineFeedCount = 0
    // Key of the password prompt at the cursor, so each prompt is offered once.
    let currentPromptKey: string | null = null
    // Key of the prompt the user already answered (typed, dismissed, or
    // filled). sudo-rs echoes `*` per key, so erasing them brings the bare
    // prompt line back; that is the same prompt and must not be offered again.
    let answeredPromptKey: string | null = null
    let offeredPrompt: { prompt: string; user: string | null; source: SudoPasswordSource } | null =
      null
    let lastPasswordFill: { prompt: string; at: number } | null = null
    // Set once a filled password is refused; stays for this session so a
    // stale password cannot burn through the remote's lockout budget.
    let savedPasswordRejected = false
    // Types the connection's login script; restarted on every (re)connect.
    let loginScriptRunner: LoginScriptRunner | null = null

    // Every state change in this effect must go through these two helpers so
    // lastConnectionState stays in sync; otherwise the output-driven
    // "connected" update is skipped as a no-op and the tab stays stuck.
    const applyConnectionState = (next: ConnectionState) => {
      lastConnectionState = next
      setConnectionState(next)
    }

    const setConnectionStateIfChanged = (next: ConnectionState) => {
      if (next === lastConnectionState) return
      applyConnectionState(next)
    }

    const clearSavedPasswordOffer = () => {
      // Invalidates any in-flight source lookup so it cannot revive the offer.
      passwordPromptCheckId += 1
      if (offeredPrompt) {
        offeredPrompt = null
        onSavedPasswordPromptChange?.(tabId, sessionNonce, null)
      }
      setSavedPasswordPrompt(null)
    }

    const answerCurrentPrompt = () => {
      if (currentPromptKey !== null) answeredPromptKey = currentPromptKey
      clearSavedPasswordOffer()
    }

    const handlePasswordPrompt = (match: PasswordPromptMatch) => {
      commandCaptureSuspended = true
      commandCaptureState = EMPTY_COMMAND_CAPTURE_STATE
      const connection = connectionRef.current
      const profileId = connection?.profileId
      const profileName = connection?.profileName

      const fill = lastPasswordFill
      lastPasswordFill = null
      if (
        fill &&
        fill.prompt === match.prompt &&
        Date.now() - fill.at < PASSWORD_REJECT_WINDOW_MS
      ) {
        savedPasswordRejected = true
        setSavedPasswordPrompt({ status: "rejected", prompt: match.prompt, user: match.user })
        onSensitivePromptRef.current?.(tabId)
        return
      }

      // A prompt naming another account (su, sudo -u with targetpw, a nested
      // login) must never receive this profile's password.
      const userMatches = match.user === null || match.user === connection?.username
      if (savedPasswordRejected || !userMatches || !(profileId || profileName)) {
        onSensitivePromptRef.current?.(tabId)
        return
      }

      const checkId = ++passwordPromptCheckId
      invoke<SudoPasswordSource | null>("get_sudo_password_source", { profileId, profileName })
        .then((source) => {
          if (disposed || checkId !== passwordPromptCheckId) return
          if (!source) {
            onSensitivePromptRef.current?.(tabId)
            return
          }
          offeredPrompt = { prompt: match.prompt, user: match.user, source }
          onSavedPasswordPromptChange?.(tabId, sessionNonce, match.prompt)
          setSavedPasswordPrompt({ status: "available", ...offeredPrompt })
        })
        .catch((err) => {
          if (disposed || checkId !== passwordPromptCheckId) return
          console.error("Failed to look up saved sudo password:", err)
          onSensitivePromptRef.current?.(tabId)
        })
    }

    // Runs after each output chunk is parsed, on the line under the cursor, so
    // escape sequences, split chunks, and tmux redraws are already resolved.
    const checkPasswordPrompt = () => {
      if (disposed || connectionRef.current?.type !== "ssh") return
      const line = readCursorLine(term.buffer.active)
      const match = matchPasswordPrompt(
        line,
        connectionRef.current.username,
        sudoPromptPatternsRef.current
      )
      const key = match ? `${lineFeedCount}:${match.prompt}` : null
      if (key === currentPromptKey) return
      currentPromptKey = key
      clearSavedPasswordOffer()
      if (match && key !== answeredPromptKey) handlePasswordPrompt(match)
    }

    const stopLoginScript = () => {
      loginScriptRunner?.stop()
      loginScriptRunner = null
    }

    const startLoginScript = () => {
      stopLoginScript()
      const connection = connectionRef.current
      if (connection?.type !== "ssh") return
      const lines = parseLoginScript(connection.loginScript)
      if (lines.length === 0) return
      loginScriptRunner = new LoginScriptRunner({
        lines,
        // Written straight to this session: a login script is never broadcast.
        send: (line) => {
          if (disposed) return
          invoke("write_pty", { tabId, sessionNonce, data: `${line}\r` }).catch(console.error)
        },
        readCursorLine: () => readCursorLine(term.buffer.active),
        isAtPasswordPrompt: () => currentPromptKey !== null,
      })
    }

    const fillSavedPassword = (): boolean => {
      const offer = offeredPrompt
      if (!offer) return false
      answerCurrentPrompt()
      commandCaptureState = EMPTY_COMMAND_CAPTURE_STATE
      commandCaptureSuspended = false

      // Only a write the backend accepted can be refused by the remote.
      const onWritten = (written: boolean | void) => {
        if (written && !disposed) lastPasswordFill = { prompt: offer.prompt, at: Date.now() }
      }
      const onInput = onInputRef.current
      if (onInput) {
        onInput({ tabId, sessionNonce, data: offer.prompt, kind: "saved-password" })
          .then(onWritten)
          .catch(console.error)
      } else {
        invoke<boolean>("write_saved_password_for_sudo", {
          tabId,
          sessionNonce,
          profileId: connectionRef.current?.profileId,
          profileName: connectionRef.current?.profileName,
          prompt: offer.prompt,
        })
          .then((written) => {
            if (!written) notifySavedPasswordNotSent(translationRef.current)
            onWritten(written)
          })
          .catch(console.error)
      }
      return true
    }

    savedPasswordPromptActionsRef.current = {
      fill: fillSavedPassword,
      dismiss: answerCurrentPrompt,
      atPasswordPrompt: () => currentPromptKey !== null,
    }

    // An AI agent reporting through tTerm's hooks; its marks on the tab go
    // with this session.
    const agentTracker = new AgentTracker({
      onStatus: (status) => setTabAgentStatus(tabId, status),
      onAnnounce: (announcement) => onAttentionRef.current?.({ kind: "agent", ...announcement }),
    })

    const commandTimer = new CommandTimer()
    const timeCommand = (data: string) => {
      const mark = parseCommandMark(data)
      if (mark?.kind === "prompt") {
        commandTimer.prompt()
        agentTracker.reset()
      } else if (mark?.kind === "output") commandTimer.start(Date.now())
      else if (mark?.kind === "done") {
        const finished = commandTimer.finish(mark.exitCode, Date.now())
        if (finished) onAttentionRef.current?.({ kind: "command", ...finished })
      }
    }

    const emitExecutedCommand = (commandText: string) => {
      const normalized = commandText.trim()
      if (!normalized) return
      commandTimer.setCommand(normalized)
      const now = Date.now()
      if (lastEmittedCommand?.text === normalized && now - lastEmittedCommand.at < 1500) return
      // A new command means the last fill's sudo run is over.
      lastPasswordFill = null
      lastEmittedCommand = { text: normalized, at: now }
      const connection = connectionRef.current
      onCommandExecutedRef.current?.({
        commandText: normalized,
        profileId: connection?.profileId,
        profileName: connection?.profileName,
        executedAt: now,
      })
    }

    const commandMarks = new CommandMarks(term)
    commandMarks.setEnabled(commandMarksEnabledRef.current)
    commandMarksRef.current = commandMarks

    const shellIntegrationDisposables = COMMAND_MARK_OSC_CODES.map((osc) =>
      term.parser.registerOscHandler(osc, (data) => {
        commandMarks.handleMark(data)
        timeCommand(data)
        const command = parseShellIntegrationCommand(data)
        if (command) emitExecutedCommand(command)
        return false
      })
    )

    // Programs asking for a notification. Returns false like the directory
    // reports below, so an OSC 9 reaches both handlers.
    const kittyNotifications = new KittyNotificationAssembler()
    const notificationDisposables = NOTIFICATION_OSC_CODES.map((osc) =>
      term.parser.registerOscHandler(osc, (data) => {
        const report = osc === 777 ? parseAgentReport(data) : null
        if (report) {
          agentTracker.report(report)
          return false
        }
        // An agent's own notifications repeat what its reports say.
        if (agentTracker.speaksForAgent) return false
        const request =
          osc === 9
            ? parseOsc9Notification(data)
            : osc === 777
              ? parseOsc777Notification(data)
              : kittyNotifications.handle(data)
        if (request) onAttentionRef.current?.({ kind: "request", ...request })
        return false
      })
    )
    const bellDisposable = term.onBell(() => onAttentionRef.current?.({ kind: "bell" }))

    // A local shell's directory, for the tab to restart in. Windows shells
    // report it themselves (OSC 7 / OSC 9;9); on macOS and Linux the backend
    // reads it from the shell process.
    let lastReportedCwd: string | null = null
    const reportCwd = (cwd: string) => {
      if (cwd === lastReportedCwd) return
      lastReportedCwd = cwd
      onCwdChangeRef.current?.(cwd)
    }
    const cwdReportDisposables = CWD_REPORT_OSC_CODES.map((osc) =>
      term.parser.registerOscHandler(osc, (data) => {
        // Over SSH these reports come from the remote host.
        if (connectionRef.current?.type === "ssh") return false
        const cwd = parseCwdReport(osc, data)
        if (cwd) reportCwd(cwd)
        return false
      })
    )
    const unlistenCwd = listen<{ sessionNonce: number; cwd: string }>(
      `pty-cwd-${tabId}`,
      (event) => {
        if (event.payload.sessionNonce === sessionNonce) reportCwd(event.payload.cwd)
      }
    )

    const lineFeedDisposable = term.onLineFeed(() => {
      lineFeedCount += 1
    })

    term.onData((data) => {
      if (waitingForReconnectRef.current) {
        waitingForReconnectRef.current = false
        applyConnectionState("connecting")
        onReconnectRequestRef.current?.()
        return
      }

      if (data === "\x1b" || data.includes("\x03")) agentTracker.interrupt()

      // Typing at the prompt means the user answers it; replies the terminal
      // itself sends (focus, cursor reports) start with ESC and do not count.
      if (!data.startsWith("\x1b")) {
        // Typing anywhere else means the user takes over from the login script.
        if (currentPromptKey === null) stopLoginScript()
        answerCurrentPrompt()
      }

      if (data.includes("\r")) {
        // Shells that send no C mark start their command here.
        if (commandMarks.isAtPrompt()) commandTimer.start(Date.now())
        commandMarks.handleEnter()
      }

      if (commandCaptureSuspended) {
        if (data.includes("\r") || data.includes("\n") || data.includes("\x03")) {
          commandCaptureSuspended = false
          commandCaptureState = EMPTY_COMMAND_CAPTURE_STATE
        }
      } else {
        const captured = captureTerminalInput(commandCaptureState, data)
        commandCaptureState = captured.state
        for (const command of captured.commands) emitExecutedCommand(command)
      }

      const onInput = onInputRef.current
      if (onInput) {
        const isMultilinePaste =
          data.includes("\n") ||
          (data.length > 1 && data.includes("\r")) ||
          data.includes("\x1b[200~")
        void onInput({
          tabId,
          sessionNonce,
          data,
          kind: isMultilinePaste ? "paste" : "keyboard",
        }).catch(console.error)
      } else {
        invoke("write_pty", { tabId, sessionNonce, data }).catch(console.error)
      }
    })

    let unlistenOutput: (() => void) | null = null
    let unlistenExit: (() => void) | null = null
    let unlistenHostPrompt: (() => void) | null = null
    let unlistenConnectionProgress: (() => void) | null = null
    // Set once a "retrying" progress arrives; the next "ready" then shows the
    // localized re-established banner instead of the plain connected state.
    let sawRetryingPhase = false

    const outputAcker = new OutputAcker((bytes) => {
      invoke("ack_pty_output", { tabId, sessionNonce, bytes }).catch(console.error)
    })

    // Hot-path terminal output. Binary chunks arrive through a Tauri Channel
    // (raw bytes, ordered, no JSON escaping); the legacy pty-output event
    // still carries cold-path status lines from jump-host connection setup.
    const handleTerminalOutput = (payload: unknown) => {
      if (disposed) return
      let text: string
      // Channel bytes are flow-controlled by the backend and must be
      // acknowledged once parsed; event strings are not.
      let channelBytes = 0
      if (typeof payload === "string") {
        text = payload
      } else if (payload instanceof Uint8Array) {
        text = decodeOutputChunk(payload)
        channelBytes = payload.byteLength
      } else if (payload instanceof ArrayBuffer) {
        text = decodeOutputChunk(new Uint8Array(payload))
        channelBytes = payload.byteLength
      } else {
        return
      }

      const scanned = scanTerminalOutput(outputScanState, text)
      outputScanState = scanned.state

      if (scanned.connecting) {
        setConnectionStateIfChanged("connecting")
      } else if (connectionRef.current?.type === "ssh" && text.length > 0) {
        setConnectionStateIfChanged("connected")
      }

      if (!isActiveRef.current && text.length > 0) onBackgroundOutputRef.current?.()

      const reservedLines = scrollbackMemory.reserve(text)
      term.write(text, () => {
        scrollbackMemory.settle(reservedLines)
        outputAcker.parsed(channelBytes)
        checkPasswordPrompt()
        loginScriptRunner?.noteOutput()
      })
    }

    const outputChannel = new Channel<ArrayBuffer>(handleTerminalOutput)

    Promise.all([
      listen<string>(`pty-output-${tabId}`, (event) => {
        handleTerminalOutput(event.payload)
      }),
      listen(`pty-exit-${tabId}`, (event) => {
        stopLoginScript()
        agentTracker.reset()
        onSessionUnavailableRef.current?.(tabId, sessionNonce, true)
        const reason = event.payload as string | null | undefined
        if (connectionRef.current?.type === "ssh") {
          const displayAddress = getConnectionDisplay(connectionRef.current, translationRef.current)
          term.writeln(`\r\n\x1b[33m${displayAddress}: session closed\x1b[0m`)
          term.writeln("\x1b[36mPress any key to reconnect\x1b[0m")

          if (reason) {
            applyConnectionState("error")
          } else {
            applyConnectionState("disconnected")
          }
          waitingForReconnectRef.current = true
        } else {
          term.writeln("\r\n\x1b[33m[Process exited]\x1b[0m")
          applyConnectionState("disconnected")
        }
      }),
      listen<HostKeyPromptState>(`ssh-hostkey-prompt-${tabId}`, async (event) => {
        // No state change here: the terminal's own connection always emits a
        // *_host_key_checking progress event first, which already sets
        // "connecting". A prompt from a side connection (SFTP) sharing this
        // tab id must leave the terminal's state alone.
        setHostKeyPrompt(event.payload)
      }),
      listen<SshConnectionProgress>(`ssh-connection-progress-${tabId}`, (event) => {
        setConnectionProgress(event.payload)
        onConnectionProgressRef.current?.(event.payload)
        if (event.payload.phase === "ready") {
          if (sawRetryingPhase) {
            sawRetryingPhase = false
            const banner = translationRef.current("sessionHeader.reconnectRestored", {
              defaultValue: "Connection re-established",
            })
            term.write(`\r\n\x1b[32m[${banner}]\x1b[0m\r\n`)
          }
          applyConnectionState("connected")
          // Each attempt opens a new shell, so the script runs again.
          startLoginScript()
        } else if (event.payload.phase === "retrying") {
          stopLoginScript()
          sawRetryingPhase = true
          applyConnectionState("reconnecting")
        } else if (event.payload.phase === "retry_exhausted") {
          // The supervisor emits pty-exit right after this; the localized
          // give-up line goes to the terminal here because the backend no
          // longer writes an English status line.
          sawRetryingPhase = false
          const giveUp = translationRef.current("sessionHeader.reconnectExhausted", {
            count: event.payload.retryMaxAttempts ?? 0,
            reason: event.payload.reason ?? "",
            defaultValue: "Automatic reconnect failed after {{count}} attempts: {{reason}}",
          })
          term.write(`\r\n\x1b[31m[${giveUp}]\x1b[0m\r\n`)
        } else if (event.payload.phase !== "failed") {
          applyConnectionState("connecting")
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

        setConnectionStateIfChanged("connecting")

        if (creatingPtyRef.current) {
          return null
        }

        creatingPtyRef.current = true
        return waitForStableFit().then(() => {
          createdPtySize = { rows: term.rows, cols: term.cols }
          return invoke<number>("create_pty", {
            tabId,
            sessionNonce,
            ...createdPtySize,
            connection: connectionRef.current,
            outputChannel,
          })
        })
      })
      .then((pid) => {
        if (pid == null) return

        if (disposed) {
          invoke("kill_pty", { tabId, sessionNonce }).catch(console.error)
          return
        }

        // Resizes are held back until the PTY exists; catch up on any that
        // happened while it was being created.
        const size = { rows: term.rows, cols: term.cols }
        lastPtySizeRef.current = size
        if (createdPtySize?.rows !== size.rows || createdPtySize.cols !== size.cols) {
          invoke("resize_pty", { tabId, sessionNonce, ...size }).catch(console.error)
        }

        if (connectionRef.current?.type !== "ssh") {
          setConnectionStateIfChanged("connected")
        }
        onPidChangeRef.current?.(pid)
      })
      .catch((error) => {
        if (disposed) return
        if (connectionRef.current?.type === "ssh") {
          applyConnectionState("error")
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
      if (surfaceRef.current) {
        resizeObserver.observe(surfaceRef.current)
      }
    }

    return () => {
      disposed = true
      outputAcker.dispose()
      passwordPromptCheckId += 1
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

      unlistenOutput?.()
      unlistenExit?.()
      unlistenHostPrompt?.()
      unlistenConnectionProgress?.()
      void unlistenCwd.then((unlisten) => unlisten())
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
      for (const disposable of notificationDisposables) disposable.dispose()
      bellDisposable.dispose()
      agentTracker.reset()
      commandMarks.dispose()
      commandMarksRef.current = null
      for (const disposable of cwdReportDisposables) disposable.dispose()
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
  ])

  useEffect(() => {
    const term = termRef.current
    if (!term || !initializedRef.current) return
    const targetRenderer = terminalRenderer ?? rendererRef.current
    if (rendererSuspendedRef.current || lastRendererRef.current === targetRenderer) return

    loadTerminalRenderer(targetRenderer, term)
    if (isActiveRef.current) {
      fitTerminalOnly()
      term.refresh(0, Math.max(0, term.rows - 1))
    }
  }, [
    fitTerminalOnly,
    initializedRef,
    isActiveRef,
    loadTerminalRenderer,
    rendererRef,
    termRef,
    terminalRenderer,
  ])

  // Before paint, so a terminal coming into sight never shows a frame without
  // its renderer.
  useLayoutEffect(() => {
    const term = termRef.current
    if (!term || !initializedRef.current) return

    if (isActive) {
      if (!rendererSuspendedRef.current) return
      rendererSuspendedRef.current = false
      loadTerminalRenderer(terminalRenderer ?? rendererRef.current, term)
      renderAllRowsNow(term)
      return
    }
    if (hiddenRendererReleaseSecs <= 0) return

    const timer = window.setTimeout(() => {
      const addon = activeRendererAddonRef.current
      if (!addon || termRef.current !== term) return
      try {
        disposeRendererAddon(term, addon)
      } catch (error) {
        console.warn("Failed to release the renderer of a hidden terminal:", error)
      }
      activeRendererAddonRef.current = null
      lastRendererRef.current = null
      rendererSuspendedRef.current = true
    }, hiddenRendererReleaseSecs * 1000)
    return () => window.clearTimeout(timer)
  }, [
    hiddenRendererReleaseSecs,
    initializedRef,
    isActive,
    loadTerminalRenderer,
    rendererRef,
    sessionNonce,
    termRef,
    terminalRenderer,
  ])

  const clearTextureAtlas = useCallback(() => {
    activeRendererAddonRef.current?.clearTextureAtlas?.()
  }, [])

  return {
    clearTextureAtlas,
  }
}
