import { Channel, invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

import type { TerminalSessionContext } from "@/components/TerminalTab/terminalLifecycleTypes"
import type { ConnectionStateUpdater } from "@/components/TerminalTab/connectionStateUpdater"
import type { SavedPasswordPrompts } from "@/components/TerminalTab/savedPasswordPrompts"
import type { CommandTracking } from "@/components/TerminalTab/commandTracking"
import { OutputAcker } from "@/components/TerminalTab/outputAck"
import { getSerialDisplay, registerSerialEcho } from "@/components/TerminalTab/serialDisplay"
import { getConnectionDisplay } from "@/components/TerminalTab/terminalTabUtils"
import type { HostKeyPromptState, SshConnectionProgress } from "@/components/TerminalTab/types"
import { setTabRemoteHostname } from "@/lib/remoteCwd"
import { createLineTimestamper } from "@/lib/serial"
import { isVaultLocked, reportVaultLocked } from "@/lib/vaultLock"
import {
  decodeOutputChunk,
  EMPTY_OUTPUT_SCAN_STATE,
  scanTerminalOutput,
  type TerminalOutputScanState,
} from "@/lib/terminalOutputScanner"

type PtySessionDeps = ConnectionStateUpdater &
  Pick<SavedPasswordPrompts, "checkPasswordPrompt" | "stopLoginScript" | "startLoginScript"> &
  Pick<CommandTracking, "agentTracker" | "commandTimer">

/**
 * Starts the session's PTY once the terminal has settled on its size, and
 * follows its output, its exit and its connection's progress.
 */
export function startPtySession(ctx: TerminalSessionContext, deps: PtySessionDeps) {
  const {
    term,
    session,
    tabId,
    sessionNonce,
    fitTerminalOnly,
    connectionRef,
    isActiveRef,
    onBackgroundOutputRef,
    scrollbackMemory,
    savedContent,
    onSessionUnavailableRef,
    translationRef,
    setHostKeyPrompt,
    setConnectionProgress,
    onConnectionProgressRef,
    creatingPtyRef,
    lastPtySizeRef,
    onPidChangeRef,
    onReconnectRequestRef,
    waitingForReconnectRef,
    setStartError,
  } = ctx
  const {
    applyConnectionState,
    setConnectionStateIfChanged,
    checkPasswordPrompt,
    stopLoginScript,
    startLoginScript,
    agentTracker,
    commandTimer,
  } = deps
  let createdPtySize: { rows: number; cols: number } | null = null
  const waitForStableFit = async () => {
    let stableFrames = 0
    for (let frame = 0; frame < 60 && stableFrames < 4; frame += 1) {
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
      if (session.disposed) return
      if (fitTerminalOnly()) stableFrames += 1
      else stableFrames = 0
    }
  }

  let outputScanState: TerminalOutputScanState = EMPTY_OUTPUT_SCAN_STATE
  const stampLines = createLineTimestamper()
  // What the send bar sent goes on a line of its own, after any half line
  // the device left, and the device's next output starts a stamped line.
  const unregisterSerialEcho = registerSerialEcho(tabId, (line) => {
    if (session.disposed) return
    const startsMidLine = term.buffer.active.cursorX > 0
    term.write(`${startsMidLine ? "\r\n" : ""}${line}\r\n`)
    stampLines.startLine()
  })
  // A serial session, like an SSH one, is a connection to reconnect.
  const isConnection = () =>
    connectionRef.current?.type === "ssh" || connectionRef.current?.type === "serial"

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
    if (session.disposed) return
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

    const serial = connectionRef.current?.type === "serial" ? connectionRef.current.serial : null
    if (serial && getSerialDisplay(tabId, serial).timestamps) text = stampLines(text)

    const scanned = scanTerminalOutput(outputScanState, text)
    outputScanState = scanned.state

    if (scanned.connecting) {
      setConnectionStateIfChanged("connecting")
    } else if (connectionRef.current?.type === "ssh" && text.length > 0) {
      setConnectionStateIfChanged("connected")
    }

    const reservedLines = scrollbackMemory.reserve(text)
    term.write(text, () => {
      // Full-screen programs such as an agent's TUI, vim or top redraw in
      // answer to focus and mouse reports, so their output is no news. They
      // ask for attention through bells, notifications or agent status. The
      // check runs once the chunk is parsed, so a program leaving the
      // alternate screen on exit still marks the tab.
      if (!isActiveRef.current && text.length > 0 && term.buffer.active.type === "normal") {
        onBackgroundOutputRef.current?.()
      }
      scrollbackMemory.settle(reservedLines)
      outputAcker.parsed(channelBytes)
      checkPasswordPrompt()
      session.loginScriptRunner?.noteOutput()
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
      commandTimer.prompt()
      onSessionUnavailableRef.current?.(tabId, sessionNonce, true)
      const reason = event.payload as string | null | undefined
      if (isConnection()) {
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
        // Tells the connected host's directory reports from those of a host
        // reached from it with ssh.
        const integration = event.payload.shellIntegration
        setTabRemoteHostname(
          tabId,
          integration?.status === "active" ? (integration.hostname ?? null) : null
        )
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
      } else if (event.payload.phase === "serial_waiting") {
        sawRetryingPhase = true
        const waiting = translationRef.current("serialControls.waiting", {
          port: event.payload.host ?? "",
          reason: event.payload.reason ?? "",
          defaultValue: "{{port}} disconnected ({{reason}}). Waiting for it to come back",
        })
        term.write(`\r\n\x1b[33m[${waiting}]\x1b[0m\r\n`)
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

      if (session.disposed) {
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
        // The last launch's content goes above the session, at the size it settled on.
        if (!session.disposed) savedContent.showSaved(translationRef.current)
        createdPtySize = { rows: term.rows, cols: term.cols }
        const connection = connectionRef.current
        return invoke<number>("create_pty", {
          tabId,
          sessionNonce,
          ...createdPtySize,
          // A serial tab reopens in the view its bar last chose.
          connection:
            connection?.type === "serial" && connection.serial
              ? {
                  ...connection,
                  serial: {
                    ...connection.serial,
                    hexView: getSerialDisplay(tabId, connection.serial).hex,
                  },
                }
              : connection,
          outputChannel,
        })
      })
    })
    .then((pid) => {
      if (pid == null) return

      if (session.disposed) {
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
      if (session.disposed) return
      if (isConnection()) {
        applyConnectionState("error")
        if (isVaultLocked(error)) {
          reportVaultLocked(() => {
            if (session.disposed) return
            applyConnectionState("connecting")
            onReconnectRequestRef.current?.()
          })
        }
      }
      term.writeln(`\x1b[31mFailed to start terminal: ${error}\x1b[0m`)
      // A local shell that didn't start gets a bar offering another one.
      if (!isConnection()) setStartError(String(error))
      if (connectionRef.current?.type === "serial") {
        // A port that didn't open, unplugged or held by another program, is
        // tried again with a key once it is free.
        term.writeln("\x1b[36mPress any key to reconnect\x1b[0m")
        waitingForReconnectRef.current = true
      }
    })
    .finally(() => {
      creatingPtyRef.current = false
    })

  const stopListening = () => {
    unregisterSerialEcho()
    unlistenOutput?.()
    unlistenExit?.()
    unlistenHostPrompt?.()
    unlistenConnectionProgress?.()
  }

  return { outputAcker, stopListening }
}
