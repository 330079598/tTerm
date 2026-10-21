import type { IBuffer, IBufferCell } from "@xterm/xterm"

import type { HighlightSpan } from "@/lib/keywordHighlight/match"

/**
 * Text read from buffer rows, with the cell behind each UTF-16 unit of it, so
 * that matches in the text can be colored on the screen. A wide character
 * such as a Chinese one is one unit over two cells; an emoji is two units
 * over one cell.
 */
export interface CellText {
  text: string
  rows: number[]
  columns: number[]
  widths: number[]
  /** The program left the cell in the default colors, so it may be highlighted. */
  plain: boolean[]
}

/** Cells of one row a highlight colors, `width` cells from column `x`. */
export interface HighlightSegment {
  row: number
  x: number
  width: number
  foreground?: string
  background?: string
}

/**
 * Reads rows `startRow` up to `endRow` (buffer rows, exclusive) as one text,
 * as a line the terminal wrapped over several rows reads. Stops once the
 * text reaches `maxLength`, and leaves out trailing blank cells.
 */
export function readCellText(
  buffer: IBuffer,
  startRow: number,
  endRow: number,
  maxLength: number,
  cell: IBufferCell
): CellText {
  const result: CellText = { text: "", rows: [], columns: [], widths: [], plain: [] }
  let text = ""
  // Units up to the last one that is not a blank cell.
  let contentLength = 0
  for (let row = startRow; row < endRow && text.length < maxLength; row += 1) {
    const line = buffer.getLine(row)
    if (!line) break
    for (let x = 0; x < line.length && text.length < maxLength; x += 1) {
      line.getCell(x, cell)
      const width = cell.getWidth()
      // The second half of a wide character.
      if (width === 0) continue
      const chars = cell.getChars()
      const plain = cell.isFgDefault() && cell.isBgDefault() && !cell.isInverse()
      const units = chars || " "
      for (let unit = 0; unit < units.length; unit += 1) {
        result.rows.push(row)
        result.columns.push(x)
        result.widths.push(width)
        result.plain.push(plain)
      }
      text += units
      if (chars && chars !== " ") contentLength = text.length
    }
  }
  result.text = text.slice(0, contentLength)
  result.rows.length = contentLength
  result.columns.length = contentLength
  result.widths.length = contentLength
  result.plain.length = contentLength
  return result
}

/**
 * The cells to color for `spans` of `cells.text`: one segment per row a span
 * covers, broken around cells the program colored itself, which keep their
 * color.
 */
export function highlightSegments(
  cells: CellText,
  spans: readonly HighlightSpan[]
): HighlightSegment[] {
  const segments: HighlightSegment[] = []
  for (const span of spans) {
    let current: HighlightSegment | null = null
    for (let index = span.start; index < span.end && index < cells.text.length; index += 1) {
      if (!cells.plain[index]) {
        current = null
        continue
      }
      const row = cells.rows[index]
      const x = cells.columns[index]
      const end = x + cells.widths[index]
      if (current && current.row === row && x <= current.x + current.width) {
        current.width = Math.max(current.width, end - current.x)
        continue
      }
      current = {
        row,
        x,
        width: end - x,
        foreground: span.foreground,
        background: span.background,
      }
      segments.push(current)
    }
  }
  return segments
}
