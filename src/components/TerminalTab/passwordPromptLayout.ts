import type { CursorCellBox } from "@/components/TerminalTab/useCursorCellBox"

/** Cells the key icon and the gap after it take. */
export const ICON_CELLS = 2

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

interface PromptBarPlacementInput {
  surfaceWidth: number
  surfaceHeight: number
  barWidth: number
  barHeight: number
  /** Where the user reads the prompt and types, or null when it is off screen. */
  input: SurfaceRect | null
}

/** The prompt line from its first cell through the key icon after the cursor. */
export function promptInputRect(box: CursorCellBox): SurfaceRect {
  return {
    left: box.rowLeft,
    top: box.top,
    right: box.left + ICON_CELLS * box.cellWidth,
    bottom: box.top + box.cellHeight,
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
