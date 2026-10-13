import { invoke } from "@tauri-apps/api/core"

import type { TerminalSessionContext } from "@/components/TerminalTab/terminalLifecycleTypes"
import type { ConnectionStateUpdater } from "@/components/TerminalTab/connectionStateUpdater"
import type { SavedPasswordPrompts } from "@/components/TerminalTab/savedPasswordPrompts"
import type { CommandTracking } from "@/components/TerminalTab/commandTracking"
import { captureTerminalInput, EMPTY_COMMAND_CAPTURE_STATE } from "@/lib/terminalCommandCapture"

type TerminalInputDeps = Pick<ConnectionStateUpdater, "applyConnectionState"> &
  Pick<SavedPasswordPrompts, "stopLoginScript" | "answerCurrentPrompt"> &
  Pick<CommandTracking, "agentTracker" | "commandMarks" | "commandTimer" | "emitExecutedCommand">

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
  } = ctx
  const {
    applyConnectionState,
    agentTracker,
    stopLoginScript,
    answerCurrentPrompt,
    commandMarks,
    commandTimer,
    emitExecutedCommand,
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
      answerCurrentPrompt()
    }

    if (data.includes("\r")) {
      agentTracker.submit()
      // Shells that send no C mark start their command here.
      if (commandMarks.isAtPrompt()) commandTimer.start(Date.now())
      commandMarks.handleEnter()
    }

    if (session.commandCaptureSuspended) {
      if (data.includes("\r") || data.includes("\n") || data.includes("\x03")) {
        session.commandCaptureSuspended = false
        session.commandCaptureState = EMPTY_COMMAND_CAPTURE_STATE
      }
    } else {
      const captured = captureTerminalInput(session.commandCaptureState, data)
      session.commandCaptureState = captured.state
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
}
