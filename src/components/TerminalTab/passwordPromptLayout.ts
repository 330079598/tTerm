import type { CursorCellBox } from "@/components/TerminalTab/useCursorCellBox"

/** Cells the key icon and the gap after it take. */
const ICON_CELLS = 2

/** Cells beside the cursor the stars need before they move off its row. */
const MIN_INLINE_MARKS = 8

/** Cells a floating chip's padding takes, half on each side. */
const CHIP_PADDING_CELLS = 1

/** Characters that take two terminal cells, as East Asian wide characters do. */
const WIDE_CHAR = /[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹏＀-｠￠-￦]/

/** Gap between the saved-password bar and the surface's edges; the bar's CSS uses the same. */
const PROMPT_BAR_INSET = 12

/** A rectangle within the terminal surface, in CSS pixels. */
export interface SurfaceRect {
  left: number
  top: number
  right: number
  bottom: number
}

export type PromptBarPlacement = "bottom" | "top"

/** Where the feedback for a password prompt goes and what it shows. */
export interface FeedbackLayout {
  /**
   * On a chip of its own on the next row, because the cursor's row has no
   * room left for it.
   */
  floating: boolean
  left: number
  top: number
  /** Width in cells the feedback may take, a chip's padding included. */
  cells: number
  /** What follows the icon: the stars, or a notice in their place. */
  text: string
  /** Whether `text` is a notice rather than the stars. */
  notice: boolean
}

interface FeedbackContent {
  /** Characters typed so far, or null when the stars are turned off. */
  typed: number | null
  /** A few words shown in the stars' place for a moment, such as that Caps Lock is on. */
  notice: string | null
}

interface PromptBarPlacementInput {
  surfaceWidth: number
  surfaceHeight: number
  barWidth: number
  barHeight: number
  /** Where the user reads the prompt and types, or null when it is off screen. */
  input: SurfaceRect | null
}

function textCells(text: string): number {
  let cells = 0
  for (const char of text) cells += WIDE_CHAR.test(char) ? 2 : 1
  return cells
}

function marksFor(typed: number, room: number): string {
  // A password longer than the room fills it, led by an ellipsis.
  return typed > room ? `…${"*".repeat(Math.max(0, room - 1))}` : "*".repeat(typed)
}

/**
 * Lays out the key and stars, or a notice, after the cursor. Near the end of
 * the row, where they would be cut off at the terminal's edge, they move onto a
 * chip on the row above, or below when the cursor is on the first row. The
 * cursor stays put while a password is typed, so the stars keep their place
 * for the whole prompt.
 */
export function feedbackLayout(
  box: CursorCellBox,
  { typed, notice }: FeedbackContent
): FeedbackLayout {
  const textFor = (room: number) => notice ?? marksFor(typed ?? 0, room)
  const needed = notice ? textCells(notice) : typed === null ? 0 : MIN_INLINE_MARKS
  const inlineRoom = box.cellsLeft - ICON_CELLS
  if (inlineRoom >= needed) {
    const text = textFor(inlineRoom)
    const layout = { left: box.left, top: box.top, cells: box.cellsLeft, text }
    return { ...layout, floating: false, notice: notice !== null }
  }

  const rowRight = box.left + box.cellsLeft * box.cellWidth
  const rowCells = Math.round((rowRight - box.rowLeft) / box.cellWidth)
  const text = textFor(Math.max(0, rowCells - ICON_CELLS - CHIP_PADDING_CELLS))
  const cells = Math.min(rowCells, ICON_CELLS + textCells(text) + CHIP_PADDING_CELLS)
  // It starts over the cursor, pulled left as far as it must to end within the row.
  const left = Math.max(
    box.rowLeft,
    Math.min(box.left - box.cellWidth, rowRight - cells * box.cellWidth)
  )
  const top = box.top >= box.cellHeight ? box.top - box.cellHeight : box.top + box.cellHeight
  return { floating: true, left, top, cells, text, notice: notice !== null }
}

/**
 * Where the user reads the prompt and looks for feedback: the prompt line
 * through the key after the cursor, and a floating chip if there is one.
 */
export function promptInputRect(box: CursorCellBox, feedback: FeedbackLayout): SurfaceRect {
  const line = {
    left: box.rowLeft,
    top: box.top,
    right: box.left + ICON_CELLS * box.cellWidth,
    bottom: box.top + box.cellHeight,
  }
  if (!feedback.floating) return line
  return {
    left: Math.min(line.left, feedback.left),
    top: Math.min(line.top, feedback.top),
    right: Math.max(line.right, feedback.left + feedback.cells * box.cellWidth),
    bottom: Math.max(line.bottom, feedback.top + box.cellHeight),
  }
}

function overlaps(a: SurfaceRect, b: SurfaceRect): boolean {
  return a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

/**
 * Keeps the saved-password bar in the bottom-right corner unless it would
 * cover the prompt line, which in a narrow pane it often does; then it moves
 * to the top-right corner. It stays at the bottom when the top would cover
 * the line too, as in a pane only a few rows tall.
 */
export function promptBarPlacement({
  surfaceWidth,
  surfaceHeight,
  barWidth,
  barHeight,
  input,
}: PromptBarPlacementInput): PromptBarPlacement {
  if (!input) return "bottom"
  const right = surfaceWidth - PROMPT_BAR_INSET
  const left = right - barWidth
  const atBottom = {
    left,
    right,
    top: surfaceHeight - PROMPT_BAR_INSET - barHeight,
    bottom: surfaceHeight - PROMPT_BAR_INSET,
  }
  const atTop = { left, right, top: PROMPT_BAR_INSET, bottom: PROMPT_BAR_INSET + barHeight }
  return overlaps(atBottom, input) && !overlaps(atTop, input) ? "top" : "bottom"
}
