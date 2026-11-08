import { invoke } from "@tauri-apps/api/core"
import type { Terminal } from "@xterm/xterm"

import type { TerminalSessionContext } from "@/components/TerminalTab/terminalLifecycleTypes"
import type { ConnectionStateUpdater } from "@/components/TerminalTab/connectionStateUpdater"
import type { SavedPasswordPrompts } from "@/components/TerminalTab/savedPasswordPrompts"
import type { CommandTracking } from "@/components/TerminalTab/commandTracking"
import type { PredictiveEcho } from "@/components/TerminalTab/predictiveEcho"
import { getSerialDisplay } from "@/components/TerminalTab/serialDisplay"
import { localEchoText } from "@/lib/serial"
import { captureTerminalInput, EMPTY_COMMAND_CAPTURE_STATE } from "@/lib/terminalCommandCapture"

type TerminalInputDeps = Pick<ConnectionStateUpdater, "applyConnectionState"> &
  Pick<SavedPasswordPrompts, "stopLoginScript" | "typeAtPrompt"> &
  Pick<CommandTracking, "agentTracker" | "commandMarks" | "commandTimer" | "emitTypedCommand"> & {
    predictiveEcho: Pick<PredictiveEcho, "noteInput">
  }

// What the compose bar is sending through the terminal's input, which hands
// it on to onData before `input` returns.
let composedInput: string | null = null

/**
 * Sends `data` from the compose bar through `term`'s own input, as typing,
 * so a single line with its Enter is told from a paste.
 */
export function inputComposed(term: Terminal, data: string) {
  composedInput = data
  try {
    term.input(data, true)
  } finally {
    composedInput = null
  }
}

/**
 * Sends what is typed to the session, noting commands as they are
 * submitted and prompts as they are answered.
 */
export function attachTerminalInput(ctx: TerminalSessionContext, deps: TerminalInputDeps) {
  const {
    term,
    session,
    tabId,
    sessionNonce,
    waitingForReconnectRef,
    onReconnectRequestRef,
    onInputRef,
    connectionRef,
  } = ctx
  const {
    applyConnectionState,
    agentTracker,
    stopLoginScript,
    typeAtPrompt,
    commandMarks,
    commandTimer,
    emitTypedCommand,
    predictiveEcho,
  } = deps
  term.onData((data) => {
    if (waitingForReconnectRef.current) {
      waitingForReconnectRef.current = false
      applyConnectionState("connecting")
      onReconnectRequestRef.current?.()
      return
    }

    if (data === "\x1b" || data.includes("\x03")) agentTracker.interrupt()
    else if (!data.startsWith("\x1b")) agentTracker.answer()

    // Typing at the prompt means the user answers it; replies the terminal
    // itself sends (focus, cursor reports) start with ESC and do not count.
    if (!data.startsWith("\x1b")) {
      // Typing anywhere else means the user takes over from the login script.
      if (session.currentPromptKey === null) stopLoginScript()
      typeAtPrompt(data)
    }

    // Lines typed into a running program, such as an agent's prompt, are not
    // shell commands. Read before Enter below starts a command.
    const inProgram = commandMarks.isRunningProgram()
    // Shells that send no C mark start their command on Enter.
    let startsCommand = false
    if (data.includes("\r")) {
      agentTracker.submit()
      startsCommand = commandMarks.handleEnter()
    }

    if (session.commandCaptureSuspended) {
      if (data.includes("\r") || data.includes("\n") || data.includes("\x03")) {
        session.commandCaptureSuspended = false
        session.commandCaptureState = EMPTY_COMMAND_CAPTURE_STATE
      }
    } else {
      const captured = captureTerminalInput(session.commandCaptureState, data)
      session.commandCaptureState = captured.state
      if (!inProgram) for (const command of captured.commands) emitTypedCommand(command)
    }
    // After the typed command is noted, so that it names what starts.
    if (startsCommand) commandTimer.start(Date.now())

    const connection = connectionRef.current
    if (connection?.type === "serial" && getSerialDisplay(tabId, connection.serial).localEcho) {
      term.write(localEchoText(data))
    }
    predictiveEcho.noteInput(data)

    const onInput = onInputRef.current
    if (onInput) {
      // One line the compose bar sends with Enter is a typed command; a paste
      // ending in a line break would run without the user pressing Enter.
      const composedLine = data === composedInput && data.indexOf("\r") === data.length - 1
      const isMultilinePaste =
        !composedLine &&
        (data.includes("\n") ||
          (data.length > 1 && data.includes("\r")) ||
          data.includes("\x1b[200~"))
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
}
