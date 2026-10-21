// Colors keywords in the terminal's output with xterm decorations, which
// leave the buffer alone: copying, logs and searches see the text as sent.
// Only the lines on screen, and a screen above and below for scrolling, carry
// decorations; lines are matched again only when their text changes.
import type {
  IBuffer,
  IBufferCell,
  IDisposable,
  IMarker,
  ITerminalAddon,
  Terminal,
} from "@xterm/xterm"

import { highlightSegments, readCellText } from "@/lib/keywordHighlight/cells"
import {
  type CompiledHighlightRule,
  matchHighlights,
  MAX_HIGHLIGHT_TEXT,
} from "@/lib/keywordHighlight/match"

/** How far up a line the terminal wrapped is followed to find its start. */
const MAX_WRAPPED_ROWS = 50

interface HighlightedLine {
  /** On the line's first row; follows the line as scrollback trims older rows. */
  marker: IMarker
  rowCount: number
  /** The line's text when it was matched. */
  text: string
  /** Its decorations and the markers of its other rows. */
  disposables: IDisposable[]
}

function lineStart(buffer: IBuffer, row: number): number {
  let start = row
  while (start > 0 && row - start < MAX_WRAPPED_ROWS && buffer.getLine(start)?.isWrapped) {
    start -= 1
  }
  return start
}

/** The row after a line starting at `start`, cut where its text gets too long to match. */
function lineEnd(buffer: IBuffer, start: number, maxRows: number): number {
  let end = start + 1
  while (end < buffer.length && end - start < maxRows && buffer.getLine(end)?.isWrapped) {
    end += 1
  }
  return end
}

function lineText(buffer: IBuffer, start: number, end: number): string {
  let text = ""
  for (let row = start; row < end; row += 1) {
    text += `${buffer.getLine(row)?.translateToString(false) ?? ""}\n`
  }
  return text
}

function disposeLine(line: HighlightedLine) {
  for (const disposable of line.disposables) disposable.dispose()
  line.marker.dispose()
}

export class KeywordHighlighter implements ITerminalAddon {
  private term: Terminal | null = null
  private rules: readonly CompiledHighlightRule[] = []
  private alternateScreen = false
  private visible = true
  private lines: HighlightedLine[] = []
  private frame: number | null = null
  private cell: IBufferCell | null = null
  private listeners: IDisposable[] = []

  activate(term: Terminal) {
    this.term = term
    const schedule = () => this.schedule()
    const restart = () => {
      // Rows reflow on resize and the other buffer has its own markers.
      this.clear()
      this.schedule()
    }
    this.listeners = [
      term.onWriteParsed(schedule),
      term.onScroll(schedule),
      term.onResize(restart),
      term.buffer.onBufferChange(restart),
    ]
  }

  /** The rules to color with, none to stop; `alternateScreen` covers full-screen programs. */
  setRules(rules: readonly CompiledHighlightRule[], alternateScreen: boolean) {
    if (rules === this.rules && alternateScreen === this.alternateScreen) return
    this.rules = rules
    this.alternateScreen = alternateScreen
    this.clear()
    this.schedule()
  }

  /** A hidden terminal is matched once it shows again. */
  setVisible(visible: boolean) {
    if (visible === this.visible) return
    this.visible = visible
    if (visible) this.schedule()
  }

  dispose() {
    if (this.frame !== null) window.cancelAnimationFrame(this.frame)
    this.frame = null
    this.clear()
    for (const listener of this.listeners) listener.dispose()
    this.listeners = []
    this.term = null
  }

  private schedule() {
    if (this.frame !== null || !this.visible || !this.term) return
    this.frame = window.requestAnimationFrame(() => {
      this.frame = null
      this.update()
    })
  }

  private clear() {
    for (const line of this.lines) disposeLine(line)
    this.lines = []
  }

  private update() {
    const term = this.term
    if (!term || !this.visible) return
    const buffer = term.buffer.active
    if (this.rules.length === 0 || (buffer.type === "alternate" && !this.alternateScreen)) {
      this.clear()
      return
    }

    const first = Math.max(0, buffer.viewportY - term.rows)
    const last = Math.min(buffer.length, buffer.viewportY + 2 * term.rows)
    const previous = new Map<number, HighlightedLine>()
    for (const line of this.lines) {
      const row = line.marker.line
      if (line.marker.isDisposed || row < 0 || previous.has(row)) disposeLine(line)
      else previous.set(row, line)
    }

    const maxRows = Math.max(1, Math.ceil(MAX_HIGHLIGHT_TEXT / Math.max(1, term.cols)))
    const lines: HighlightedLine[] = []
    for (let start = lineStart(buffer, first); start < last;) {
      const end = lineEnd(buffer, start, maxRows)
      const text = lineText(buffer, start, end)
      const kept = previous.get(start)
      previous.delete(start)
      if (kept && kept.text === text && kept.rowCount === end - start) {
        lines.push(kept)
      } else {
        if (kept) disposeLine(kept)
        const line = this.highlightLine(term, start, end, text)
        if (line) lines.push(line)
      }
      start = end
    }
    // Lines scrolled away, trimmed or rewrapped.
    for (const line of previous.values()) disposeLine(line)
    this.lines = lines
  }

  private highlightLine(
    term: Terminal,
    start: number,
    end: number,
    text: string
  ): HighlightedLine | null {
    const buffer = term.buffer.active
    // Markers are placed relative to the cursor's row.
    const cursorRow = buffer.baseY + buffer.cursorY
    const marker = term.registerMarker(start - cursorRow)
    if (!marker) return null
    const line: HighlightedLine = { marker, rowCount: end - start, text, disposables: [] }

    this.cell ??= buffer.getNullCell()
    const cells = readCellText(buffer, start, end, MAX_HIGHLIGHT_TEXT, this.cell)
    const segments = highlightSegments(cells, matchHighlights(cells.text, this.rules))
    const rowMarkers = new Map<number, IMarker>([[start, marker]])
    for (const segment of segments) {
      let rowMarker = rowMarkers.get(segment.row)
      if (!rowMarker) {
        rowMarker = term.registerMarker(segment.row - cursorRow)
        if (!rowMarker) continue
        rowMarkers.set(segment.row, rowMarker)
        line.disposables.push(rowMarker)
      }
      const decoration = term.registerDecoration({
        marker: rowMarker,
        x: segment.x,
        width: segment.width,
        foregroundColor: segment.foreground,
        backgroundColor: segment.background,
        layer: "bottom",
      })
      // Decorations go before their markers, which disposing a line follows.
      if (decoration) line.disposables.unshift(decoration)
    }
    return line
  }
}
