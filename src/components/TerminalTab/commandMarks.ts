// Prompts and commands located by shell integration marks (OSC 133, and
// VS Code's OSC 633 with the same letters): A where a prompt starts, C where a
// command's output starts, D with its exit status once it ends. tTerm's own
// scripts for local Windows shells send them (see
// src-tauri/src/terminal/shell_integration.rs); remote shells may too.
import type { IBufferLine, IDecoration, IDisposable, IMarker, Terminal } from "@xterm/xterm"

export const COMMAND_MARK_OSC_CODES = [133, 633] as const

export type CommandMark =
  { kind: "prompt" } | { kind: "output" } | { kind: "done"; exitCode?: number }

/** The mark in an OSC 133 / 633 payload such as `A`, `C` or `D;1`. */
export function parseCommandMark(data: string): CommandMark | null {
  const [letter, param] = data.split(";", 2)
  if (letter === "A") return { kind: "prompt" }
  if (letter === "C") return { kind: "output" }
  if (letter !== "D") return null
  const exitCode = param !== undefined && /^-?\d+$/.test(param) ? Number(param) : undefined
  return { kind: "done", exitCode }
}

/** Lines of the buffer joined as the user sees them: wrapped rows run on. */
export function joinBufferLines(
  lines: readonly Pick<IBufferLine, "isWrapped" | "translateToString">[]
) {
  let text = ""
  lines.forEach((line, index) => {
    if (index > 0 && !line.isWrapped) text += "\n"
    text += line.translateToString(true)
  })
  return text.replace(/\s+$/, "")
}

/** Colors of a finished command, on the scrollbar and beside its prompt. */
const SUCCEEDED_COLOR = "#22c55e"
const FAILED_COLOR = "#ef4444"

type CommandEntry = {
  prompt: IMarker
  output?: {
    marker: IMarker
    /** Set from Enter on the command's line: the output starts on the next one. */
    nextLine: boolean
  }
  end?: {
    marker: IMarker
    /** The output's last line had no line break, so the marked line is part of it. */
    inclusive: boolean
  }
  exitCode?: number
  decoration?: IDecoration
}

/**
 * Tracks the prompts and commands of one terminal and acts on them: jumping
 * between prompts, copying a command's output, coloring finished commands on
 * the scrollbar and beside their prompts.
 */
export class CommandMarks implements IDisposable {
  private entries: CommandEntry[] = []
  /** The shell sends C marks, so Enter need not stand in for them. */
  private sendsOutputMarks = false
  /** The shell sends D marks when commands finish (all but cmd). */
  private sendsDoneMarks = false
  private enabled = true
  /** The prompt the last jump went to, while the view still shows it there. */
  private jumped: { prompt: IMarker; viewportY: number } | null = null

  constructor(private readonly term: Terminal) {}

  setEnabled(enabled: boolean) {
    if (enabled === this.enabled) return
    this.enabled = enabled
    if (!enabled) this.clear()
  }

  handleMark(data: string) {
    if (!this.enabled || this.term.buffer.active.type !== "normal") return
    const mark = parseCommandMark(data)
    if (!mark) return
    if (mark.kind === "prompt") this.startPrompt()
    else if (mark.kind === "output") this.startOutput()
    else {
      this.sendsDoneMarks = true
      this.finishCommand(mark.exitCode)
    }
  }

  /**
   * The shell is waiting at a prompt, so input reaches its line editor rather
   * than a running program. Only known for shells that mark finished commands:
   * bash, zsh, fish and PowerShell, whose editors also redraw on Ctrl+L
   * (unlike cmd).
   */
  isAtPrompt(): boolean {
    if (!this.enabled || !this.sendsDoneMarks || this.term.buffer.active.type !== "normal") {
      return false
    }
    const entry = this.current()
    return entry !== undefined && !entry.output && !entry.end
  }

  /**
   * Enter pressed. A shell that sends no C mark (PowerShell, cmd) starts its
   * output on the line after the one the command was typed on.
   */
  handleEnter() {
    if (!this.enabled || this.sendsOutputMarks || this.term.buffer.active.type !== "normal") return
    const entry = this.current()
    if (!entry || entry.output || entry.end) return
    const marker = this.term.registerMarker(0)
    if (marker) entry.output = { marker, nextLine: true }
  }

  /** Scrolls the previous prompt to the top. Returns false when no prompt is known. */
  scrollToPreviousPrompt(): boolean {
    const entries = this.liveEntries()
    if (entries.length === 0) return false
    const buffer = this.term.buffer.active
    const jumpedLine = this.jumpedPromptLine()
    let reference = jumpedLine ?? buffer.viewportY
    if (jumpedLine === null && buffer.viewportY >= buffer.baseY) {
      // At the bottom the latest prompt is in view: go to the one before it,
      // or to it while its command still runs.
      const last = entries[entries.length - 1]
      reference = last.output && !last.end ? Number.POSITIVE_INFINITY : last.prompt.line
    }
    const target = [...entries].reverse().find((entry) => entry.prompt.line < reference)
    if (target) this.jumpTo(target.prompt)
    return true
  }

  /** Scrolls the next prompt to the top, or to the bottom past the last one. */
  scrollToNextPrompt(): boolean {
    const entries = this.liveEntries()
    if (entries.length === 0) return false
    const reference = this.jumpedPromptLine() ?? this.term.buffer.active.viewportY
    const target = entries.find((entry) => entry.prompt.line > reference)
    if (target && target.prompt.line < this.term.buffer.active.baseY) {
      this.jumpTo(target.prompt)
    } else {
      this.jumped = null
      this.term.scrollToBottom()
    }
    return true
  }

  /** The output of the last command that finished, as lines of the buffer. */
  lastOutputRange(): { start: number; end: number } | null {
    for (const entry of this.liveEntries().reverse()) {
      if (!entry.output || !entry.end || entry.exitCode === undefined) continue
      if (entry.output.marker.isDisposed || entry.end.marker.isDisposed) return null
      const start = entry.output.marker.line + (entry.output.nextLine ? 1 : 0)
      const end = entry.end.marker.line - (entry.end.inclusive ? 0 : 1)
      return { start, end }
    }
    return null
  }

  /** Text of the last finished command's output; empty when it printed nothing. */
  lastOutputText(): string | null {
    const range = this.lastOutputRange()
    if (!range) return null
    const buffer = this.term.buffer.active
    const lines: IBufferLine[] = []
    for (let row = range.start; row <= range.end; row += 1) {
      const line = buffer.getLine(row)
      if (line) lines.push(line)
    }
    return joinBufferLines(lines)
  }

  /** Selects the last finished command's output. */
  selectLastOutput(): boolean {
    const range = this.lastOutputRange()
    if (!range || range.end < range.start) return false
    this.term.selectLines(range.start, range.end)
    return true
  }

  dispose() {
    this.clear()
  }

  /**
   * Scrolling stops at the bottom, so a prompt near it may not reach the top;
   * the next jump then still counts from that prompt.
   */
  private jumpTo(prompt: IMarker) {
    this.term.scrollToLine(prompt.line)
    this.jumped = { prompt, viewportY: this.term.buffer.active.viewportY }
  }

  private jumpedPromptLine(): number | null {
    const jumped = this.jumped
    if (
      !jumped ||
      jumped.prompt.isDisposed ||
      jumped.viewportY !== this.term.buffer.active.viewportY
    ) {
      this.jumped = null
      return null
    }
    return jumped.prompt.line
  }

  private startPrompt() {
    const previous = this.current()
    if (previous && !previous.end) this.end(previous)
    const prompt = this.term.registerMarker(0)
    if (!prompt) return
    const entry: CommandEntry = { prompt }
    this.entries.push(entry)
    // The scrollback dropped the prompt's line, or the terminal was cleared.
    prompt.onDispose(() => this.remove(entry))
  }

  private startOutput() {
    this.sendsOutputMarks = true
    const entry = this.current()
    // Only the first C counts; one from Enter gives way to it.
    if (!entry || entry.end || (entry.output && !entry.output.nextLine)) return
    entry.output?.marker.dispose()
    const marker = this.term.registerMarker(0)
    entry.output = marker ? { marker, nextLine: false } : undefined
  }

  private finishCommand(exitCode: number | undefined) {
    const entry = this.current()
    if (!entry || entry.end) return
    this.end(entry)
    // A prompt left without running anything (Enter on an empty line) ends
    // with no output mark; its status is the previous command's.
    if (!entry.output || exitCode === undefined) return
    entry.exitCode = exitCode
    const color = exitCode === 0 ? SUCCEEDED_COLOR : FAILED_COLOR
    const decoration = this.term.registerDecoration({
      marker: entry.prompt,
      overviewRulerOptions: { color, position: "left" },
    })
    // The scrollbar merges nearby marks of one color; a dot in the left
    // padding beside each prompt keeps every command apart (TerminalTab.css).
    decoration?.onRender((element) => {
      element.classList.add("command-mark-dot")
      element.style.color = color
    })
    entry.decoration = decoration
  }

  private end(entry: CommandEntry) {
    const marker = this.term.registerMarker(0)
    if (marker) entry.end = { marker, inclusive: this.term.buffer.active.cursorX > 0 }
  }

  private current(): CommandEntry | undefined {
    const entry = this.entries[this.entries.length - 1]
    return entry && !entry.prompt.isDisposed ? entry : undefined
  }

  private liveEntries() {
    return this.entries.filter((entry) => !entry.prompt.isDisposed)
  }

  private remove(entry: CommandEntry) {
    const index = this.entries.indexOf(entry)
    if (index < 0) return
    this.entries.splice(index, 1)
    this.disposeEntry(entry)
  }

  private disposeEntry(entry: CommandEntry) {
    entry.decoration?.dispose()
    entry.output?.marker.dispose()
    entry.end?.marker.dispose()
    entry.prompt.dispose()
  }

  private clear() {
    const entries = this.entries
    this.entries = []
    this.jumped = null
    for (const entry of entries) this.disposeEntry(entry)
  }
}
