/** Quiet time after a line that looks like a shell prompt before the next command is typed. */
export const LOGIN_SCRIPT_PROMPT_IDLE_MS = 250
/**
 * Quiet time after any other output. Prompts that do not end in a usual
 * prompt character still get the script, just later.
 */
export const LOGIN_SCRIPT_FALLBACK_IDLE_MS = 1500

/** Characters a shell prompt usually ends with: sh/bash, root, zsh, fish/PowerShell, themes. */
const SHELL_PROMPT_END = /[$#%>❯➜λ»]\s*$/

/** The commands of a login script: one per line, blank lines skipped. */
export function parseLoginScript(script: string | null | undefined): string[] {
  return (script ?? "")
    .split(/\r\n|\r|\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim().length > 0)
}

/** Whether the text before the cursor looks like a shell waiting for a command. */
export function looksLikeShellPrompt(line: string): boolean {
  return SHELL_PROMPT_END.test(line)
}

export interface LoginScriptRunnerOptions {
  lines: readonly string[]
  /** Types one command; the runner adds nothing, so the line ending is the caller's. */
  send: (line: string) => void
  /** The text before the cursor, read when the output goes quiet. */
  readCursorLine: () => string
  /** True while a password prompt waits; the user (or sudo autofill) answers it first. */
  isAtPasswordPrompt: () => boolean
  promptIdleMs?: number
  fallbackIdleMs?: number
}

/**
 * Types a login script into a shell one command at a time. Each command waits
 * until the output goes quiet, so it reaches the shell rather than a program
 * still starting (a `sudo -i` password prompt must not receive the next line).
 */
export class LoginScriptRunner {
  private readonly options: LoginScriptRunnerOptions
  private nextIndex = 0
  private timer: ReturnType<typeof setTimeout> | null = null
  private stopped = false

  constructor(options: LoginScriptRunnerOptions) {
    this.options = options
    if (options.lines.length === 0) {
      this.stopped = true
      return
    }
    // The prompt may already be on screen when the session reports ready.
    this.noteOutput()
  }

  get done(): boolean {
    return this.stopped
  }

  /** Call after each output chunk is on screen. */
  noteOutput(): void {
    if (this.stopped) return
    if (this.timer !== null) clearTimeout(this.timer)
    const delay = looksLikeShellPrompt(this.options.readCursorLine())
      ? (this.options.promptIdleMs ?? LOGIN_SCRIPT_PROMPT_IDLE_MS)
      : (this.options.fallbackIdleMs ?? LOGIN_SCRIPT_FALLBACK_IDLE_MS)
    this.timer = setTimeout(() => this.typeNextLine(), delay)
  }

  stop(): void {
    this.stopped = true
    if (this.timer !== null) clearTimeout(this.timer)
    this.timer = null
  }

  private typeNextLine(): void {
    this.timer = null
    // Wait for the output that follows the user's answer.
    if (this.stopped || this.options.isAtPasswordPrompt()) return
    const line = this.options.lines[this.nextIndex]
    this.nextIndex += 1
    if (this.nextIndex >= this.options.lines.length) this.stop()
    this.options.send(line)
  }
}
