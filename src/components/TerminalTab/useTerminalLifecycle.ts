import { useCallback, useEffect, useRef } from "react"
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

import { getConnectionDisplay } from "@/components/TerminalTab/terminalTabUtils"
import type {
  ConnectionState,
  HostKeyPromptState,
  SshConnectionProgress,
  TerminalTabProps,
} from "@/components/TerminalTab/types"
import { resolveScrollbackLines } from "@/lib/scrollback"
import type { TerminalRenderer } from "@/contexts/ConfigContext"
import { safePreloadFont, updateCanvasFontHostFont } from "@/lib/canvasFontHost"
import {
  decodeOutputChunk,
  EMPTY_OUTPUT_SCAN_STATE,
  scanTerminalOutput,
  type TerminalOutputScanState,
} from "@/lib/terminalOutputScanner"
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
  activateFitTimerRef: React.MutableRefObject<number | null>
  connectionRef: React.MutableRefObject<TerminalTabProps["connection"]>
  containerRef: React.RefObject<HTMLDivElement>
  creatingPtyRef: React.MutableRefObject<boolean>
  fitAddonRef: React.MutableRefObject<FitAddon | null>
  fitTerminalOnly: () => boolean
  initializedRef: React.MutableRefObject<boolean>
  configCursorStyleRef?: React.MutableRefObject<Terminal["options"]["cursorStyle"]>
  configFontFamilyRef?: React.MutableRefObject<string>
  configFontSizeRef?: React.MutableRefObject<number>
  configScrollbackLinesRef?: React.MutableRefObject<number>
  configTerminalRendererRef?: React.MutableRefObject<TerminalRenderer>
  terminalThemeRef?: React.MutableRefObject<NonNullable<Terminal["options"]["theme"]>>
  initialCursorStyle?: React.MutableRefObject<Terminal["options"]["cursorStyle"]>
  initialFontFamily?: React.MutableRefObject<string>
  initialFontSize?: React.MutableRefObject<number>
  initialScrollbackLines?: React.MutableRefObject<number>
  initialTerminalRenderer?: React.MutableRefObject<TerminalRenderer>
  initialTerminalThemeRef?: React.MutableRefObject<NonNullable<Terminal["options"]["theme"]>>
  terminalRenderer?: TerminalRenderer
  isActiveRef: React.MutableRefObject<boolean>
  lastPtySizeRef: React.MutableRefObject<{ rows: number; cols: number } | null>
  onPidChangeRef: React.MutableRefObject<TerminalTabProps["onPidChange"]>
  onInputRef: React.MutableRefObject<TerminalTabProps["onInput"]>
  onCommandExecutedRef: React.MutableRefObject<TerminalTabProps["onCommandExecuted"]>
  onReconnectRequestRef: React.MutableRefObject<TerminalTabProps["onReconnectRequest"]>
  onSavedPasswordPromptChangeRef: React.MutableRefObject<
    TerminalTabProps["onSavedPasswordPromptChange"]
  >
  onSessionUnavailableRef: React.MutableRefObject<TerminalTabProps["onSessionUnavailable"]>
  onSensitivePromptRef: React.MutableRefObject<TerminalTabProps["onSensitivePrompt"]>
  passwordPromptActiveRef: React.MutableRefObject<boolean>
  resizeObserverRef: React.MutableRefObject<ResizeObserver | null>
  resizePtySyncTimerRef: React.MutableRefObject<number | null>
  resizeRafRef: React.MutableRefObject<number | null>
  scheduleFitDuringResize: () => void
  surfaceRef: React.RefObject<HTMLDivElement>
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
  configCursorStyleRef,
  configFontFamilyRef,
  configFontSizeRef,
  configScrollbackLinesRef,
  configTerminalRendererRef,
  terminalThemeRef,
  initialCursorStyle,
  initialFontFamily,
  initialFontSize,
  initialScrollbackLines,
  initialTerminalRenderer,
  initialTerminalThemeRef,
  isActiveRef,
  lastPtySizeRef,
  onPidChangeRef,
  onInputRef,
  onCommandExecutedRef,
  onReconnectRequestRef,
  onSavedPasswordPromptChangeRef,
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
  terminalRenderer,
  termRef,
  waitingForReconnectRef,
}: UseTerminalLifecycleOptions) {
  const activeRendererAddonRef = useRef<ActiveRendererAddon | null>(null)
  const lastRendererRef = useRef<TerminalRenderer | null>(null)

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
          activeRendererAddonRef.current.dispose()
        } catch (error) {
          console.error("Failed to dispose active terminal renderer addon:", error)
        }
        activeRendererAddonRef.current = null
      }

      if (targetRenderer === "webgl") {
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
            try {
              const canvasAddon = new CanvasAddon()
              term.loadAddon(canvasAddon)
              activeRendererAddonRef.current = canvasAddon
              lastRendererRef.current = "canvas"
            } catch (canvasErr) {
              console.error("Failed to load canvas fallback after WebGL context loss:", canvasErr)
            }
          })
          term.loadAddon(webglAddon)
          activeRendererAddonRef.current = webglAddon
          lastRendererRef.current = "webgl"
          return
        } catch (error) {
          console.warn(
            "WebGL not supported in this environment; falling back to canvas renderer",
            error
          )
        }
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
    passwordPromptActiveRef.current = false

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
      scrollback: resolveScrollbackLines(scrollbackLinesRef.current),
      fontSize: fontSizeRef.current,
      fontFamily: fontFamilyRef.current,
      fontWeight: "normal",
      fontWeightBold: "bold",
      letterSpacing: 0,
      lineHeight: 1.0,
      theme: themeRef.current,
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

    termRef.current = term
    fitAddonRef.current = fitAddon
    searchAddonRef.current = searchAddon
    searchResultsDisposableRef.current = searchAddon.onDidChangeResults((results) => {
      setSearchResults(results)
    })

    container.replaceChildren()
    term.open(container)

    const effectiveRenderer = terminalRenderer ?? rendererRef.current
    loadTerminalRenderer(effectiveRenderer, term)

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
    let activeSudoPromptUser: string | null = null
    let lastConnectionState: ConnectionState = "connecting"

    const setConnectionStateIfChanged = (next: ConnectionState) => {
      if (next === lastConnectionState) return
      lastConnectionState = next
      setConnectionState(next)
    }

    const handleSudoPrompt = (promptUsername: string) => {
      const currentPasswordPromptCheckId = ++passwordPromptCheckId
      commandCaptureSuspended = true
      commandCaptureState = EMPTY_COMMAND_CAPTURE_STATE
      const savedUsername = connectionRef.current?.username
      const profileId = connectionRef.current?.profileId
      const profileName = connectionRef.current?.profileName

      if (savedUsername && promptUsername === savedUsername && profileName) {
        passwordPromptActiveRef.current = true
        invoke<boolean>("has_saved_password", {
          profileId,
          profileName,
        })
          .then((hasPassword) => {
            if (disposed || currentPasswordPromptCheckId !== passwordPromptCheckId) return
            if (hasPassword) {
              passwordPromptActiveRef.current = true
              onSavedPasswordPromptChange?.(tabId, sessionNonce, true)
              const pasteHint =
                "\x1b[100m\x1b[36m tTerm \x1b[0m " +
                "\x1b[90mPress Enter to paste saved password\x1b[0m"
              term.write(pasteHint)
            } else {
              passwordPromptActiveRef.current = false
              onSensitivePromptRef.current?.(tabId)
            }
          })
          .catch((err) => {
            if (disposed || currentPasswordPromptCheckId !== passwordPromptCheckId) return
            passwordPromptActiveRef.current = false
            console.error("Failed to get saved password:", err)
            onSensitivePromptRef.current?.(tabId)
          })
      } else {
        onSensitivePromptRef.current?.(tabId)
      }
    }

    const emitExecutedCommand = (commandText: string) => {
      const normalized = commandText.trim()
      if (!normalized) return
      const now = Date.now()
      if (lastEmittedCommand?.text === normalized && now - lastEmittedCommand.at < 1500) return
      lastEmittedCommand = { text: normalized, at: now }
      const connection = connectionRef.current
      onCommandExecutedRef.current?.({
        commandText: normalized,
        profileId: connection?.profileId,
        profileName: connection?.profileName,
        executedAt: now,
      })
    }

    const shellIntegrationDisposables = [133, 633].map((osc) =>
      term.parser.registerOscHandler(osc, (data) => {
        const command = parseShellIntegrationCommand(data)
        if (command) emitExecutedCommand(command)
        return false
      })
    )

    term.onData((data) => {
      if (waitingForReconnectRef.current) {
        waitingForReconnectRef.current = false
        setConnectionState("connecting")
        onReconnectRequestRef.current?.()
        return
      }

      if (passwordPromptActiveRef.current) {
        passwordPromptCheckId += 1
        term.write("\r\x1b[K")
        passwordPromptActiveRef.current = false
        activeSudoPromptUser = null
        onSavedPasswordPromptChange?.(tabId, sessionNonce, false)

        if (data === "\r") {
          commandCaptureState = EMPTY_COMMAND_CAPTURE_STATE
          commandCaptureSuspended = false
          const onInput = onInputRef.current
          if (onInput) {
            void onInput({ tabId, sessionNonce, data: "", kind: "saved-password" }).catch(
              console.error
            )
          } else {
            invoke("write_saved_password_for_sudo", {
              tabId,
              sessionNonce,
              profileId: connectionRef.current?.profileId,
              profileName: connectionRef.current?.profileName,
            }).catch(console.error)
          }
          return
        }

        invoke("write_pty", { tabId, sessionNonce, data }).catch(console.error)
        return
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

    // Hot-path terminal output. Binary chunks arrive through a Tauri Channel
    // (raw bytes, ordered, no JSON escaping); the legacy pty-output event
    // still carries cold-path status lines from jump-host connection setup.
    const handleTerminalOutput = (payload: unknown) => {
      if (disposed) return
      let text: string
      if (typeof payload === "string") {
        text = payload
      } else if (payload instanceof Uint8Array) {
        text = decodeOutputChunk(payload)
      } else if (payload instanceof ArrayBuffer) {
        text = decodeOutputChunk(new Uint8Array(payload))
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

      if (scanned.sudoPromptUser !== null) {
        // A \r or \n in this chunk means the prompt line started fresh in it
        // (e.g. sudo retrying after a wrong password), so re-arm even for the
        // same user; without it, only a username change re-triggers.
        const promptLineStartedInChunk = /[\r\n]/.test(text)
        if (scanned.sudoPromptUser !== activeSudoPromptUser || promptLineStartedInChunk) {
          handleSudoPrompt(scanned.sudoPromptUser)
        }
        activeSudoPromptUser = scanned.sudoPromptUser
      } else if (activeSudoPromptUser !== null) {
        activeSudoPromptUser = null
        // Invalidate any in-flight has_saved_password reply so it cannot
        // revive the prompt after the output stream moved past it.
        passwordPromptCheckId += 1
        if (passwordPromptActiveRef.current) {
          passwordPromptActiveRef.current = false
          onSavedPasswordPromptChange?.(tabId, sessionNonce, false)
        }
      }

      term.write(text)
    }

    const outputChannel = new Channel<ArrayBuffer>(handleTerminalOutput)

    Promise.all([
      listen<string>(`pty-output-${tabId}`, (event) => {
        handleTerminalOutput(event.payload)
      }),
      listen(`pty-exit-${tabId}`, (event) => {
        onSessionUnavailableRef.current?.(tabId, sessionNonce, true)
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
          setConnectionState("disconnected")
        }
      }),
      listen<HostKeyPromptState>(`ssh-hostkey-prompt-${tabId}`, async (event) => {
        setHostKeyPrompt(event.payload)
        setConnectionState("connecting")
      }),
      listen<SshConnectionProgress>(`ssh-connection-progress-${tabId}`, (event) => {
        setConnectionProgress(event.payload)
        if (event.payload.phase === "ready") {
          if (sawRetryingPhase) {
            sawRetryingPhase = false
            const banner = translationRef.current("sessionHeader.reconnectRestored", {
              defaultValue: "Connection re-established",
            })
            term.write(`\r\n\x1b[32m[${banner}]\x1b[0m\r\n`)
          }
          setConnectionState("connected")
        } else if (event.payload.phase === "retrying") {
          sawRetryingPhase = true
          setConnectionState("reconnecting")
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

        setConnectionStateIfChanged("connecting")

        if (creatingPtyRef.current) {
          return null
        }

        creatingPtyRef.current = true
        return waitForStableFit().then(() =>
          invoke<number>("create_pty", {
            tabId,
            sessionNonce,
            rows: term.rows,
            cols: term.cols,
            connection: connectionRef.current,
            outputChannel,
          })
        )
      })
      .then((pid) => {
        if (pid == null) return

        if (disposed) {
          invoke("kill_pty", { tabId, sessionNonce }).catch(console.error)
          return
        }

        if (connectionRef.current?.type !== "ssh") {
          setConnectionStateIfChanged("connected")
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
      if (surfaceRef.current) {
        resizeObserver.observe(surfaceRef.current)
      }
    }

    return () => {
      disposed = true
      passwordPromptCheckId += 1

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
      invoke("kill_pty", { tabId, sessionNonce }).catch(console.error)
      searchResultsDisposableRef.current?.dispose()
      searchResultsDisposableRef.current = null
      searchAddonRef.current = null
      if (typeof document !== "undefined") {
        document.fonts?.removeEventListener?.("loadingdone", handleFontsLoaded)
      }
      if (activeRendererAddonRef.current) {
        try {
          activeRendererAddonRef.current.dispose()
        } catch (disposeErr) {
          console.warn("Failed to dispose active renderer addon during unmount:", disposeErr)
        }
        activeRendererAddonRef.current = null
      }
      lastRendererRef.current = null
      term.dispose()
      termRef.current = null
      fitAddonRef.current = null
      initializedRef.current = false
      lastPtySizeRef.current = null
      creatingPtyRef.current = false
      waitingForReconnectRef.current = false
      passwordPromptActiveRef.current = false
      onSavedPasswordPromptChange?.(tabId, sessionNonce, false)
      for (const disposable of scrollbackDisposables) disposable.dispose()
      for (const disposable of shellIntegrationDisposables) disposable.dispose()
      container.classList.remove("xterm-has-scrollback")
      container.replaceChildren()
    }
  }, [
    activateFitTimerRef,
    connectionRef,
    containerRef,
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
    onInputRef,
    onCommandExecutedRef,
    onReconnectRequestRef,
    onSavedPasswordPromptChangeRef,
    onSessionUnavailableRef,
    onSensitivePromptRef,
    passwordPromptActiveRef,
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
    if (lastRendererRef.current === targetRenderer) return

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

  const clearTextureAtlas = useCallback(() => {
    activeRendererAddonRef.current?.clearTextureAtlas?.()
  }, [])

  return {
    clearTextureAtlas,
  }
}
