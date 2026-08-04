export interface TerminalCommandCaptureState {
  text: string
  cursor: number
  bracketedPaste: boolean
  valid: boolean
}

export interface TerminalCommandCaptureResult {
  state: TerminalCommandCaptureState
  commands: string[]
}

export function isSaveCommandShortcut(event: {
  ctrlKey: boolean
  metaKey: boolean
  shiftKey: boolean
  key: string
}) {
  return (event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === "s"
}

export const EMPTY_COMMAND_CAPTURE_STATE: TerminalCommandCaptureState = {
  text: "",
  cursor: 0,
  bracketedPaste: false,
  valid: true,
}

function insertAtCursor(state: TerminalCommandCaptureState, value: string) {
  state.text = `${state.text.slice(0, state.cursor)}${value}${state.text.slice(state.cursor)}`
  state.cursor += value.length
}

export function captureTerminalInput(
  current: TerminalCommandCaptureState,
  data: string
): TerminalCommandCaptureResult {
  const state = { ...current }
  const commands: string[] = []

  for (let index = 0; index < data.length; ) {
    if (data.startsWith("\x1b[200~", index)) {
      state.bracketedPaste = true
      index += 6
      continue
    }
    if (data.startsWith("\x1b[201~", index)) {
      state.bracketedPaste = false
      index += 6
      continue
    }

    const sequence = [
      "\x1b[1~",
      "\x1b[4~",
      "\x1b[3~",
      "\x1b[H",
      "\x1b[F",
      "\x1b[D",
      "\x1b[C",
      "\x1b[A",
      "\x1b[B",
    ].find((candidate) => data.startsWith(candidate, index))
    if (sequence) {
      if (sequence === "\x1b[D") state.cursor = Math.max(0, state.cursor - 1)
      else if (sequence === "\x1b[C") state.cursor = Math.min(state.text.length, state.cursor + 1)
      else if (sequence === "\x1b[H" || sequence === "\x1b[1~") state.cursor = 0
      else if (sequence === "\x1b[F" || sequence === "\x1b[4~") state.cursor = state.text.length
      else if (sequence === "\x1b[3~" && state.cursor < state.text.length) {
        state.text = `${state.text.slice(0, state.cursor)}${state.text.slice(state.cursor + 1)}`
      } else if (sequence === "\x1b[A" || sequence === "\x1b[B") {
        state.text = ""
        state.cursor = 0
        state.valid = false
      }
      index += sequence.length
      continue
    }

    const char = data[index]
    index += 1

    if (char === "\r" || char === "\n") {
      if (state.bracketedPaste) {
        insertAtCursor(state, "\n")
      } else {
        const command = state.text.trim()
        if (state.valid && command) commands.push(command)
        state.text = ""
        state.cursor = 0
        state.valid = true
      }
      continue
    }
    if (char === "\x7f" || char === "\b") {
      if (state.cursor > 0) {
        state.text = `${state.text.slice(0, state.cursor - 1)}${state.text.slice(state.cursor)}`
        state.cursor -= 1
      }
      continue
    }
    if (char === "\x15" || char === "\x03") {
      state.text = ""
      state.cursor = 0
      state.valid = true
      continue
    }
    if (char === "\x17") {
      const before = state.text.slice(0, state.cursor).replace(/\s*\S+\s*$/, "")
      state.text = `${before}${state.text.slice(state.cursor)}`
      state.cursor = before.length
      continue
    }
    if (char === "\t" || char === "\x1b") {
      state.valid = false
      continue
    }
    if (char >= " ") insertAtCursor(state, char)
  }

  return { state, commands }
}

export function parseShellIntegrationCommand(data: string): string | null {
  const separatorIndex = data.indexOf(";")
  if (separatorIndex < 0) return null
  const marker = data.slice(0, separatorIndex)
  const command = data.slice(separatorIndex + 1).trim()
  if ((marker === "E" || marker === "C") && command) {
    return command
  }
  return null
}
