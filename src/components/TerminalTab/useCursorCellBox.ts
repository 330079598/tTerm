import { useLayoutEffect, useState, type RefObject } from "react"
import type { Terminal } from "@xterm/xterm"

/** Where the cell after the cursor sits within the terminal surface, in CSS pixels. */
export interface CursorCellBox {
  left: number
  top: number
  cellWidth: number
  cellHeight: number
  /** Cells from that one to the end of the row. */
  cellsLeft: number
}

interface Rect {
  left: number
  top: number
  width: number
  height: number
}

interface CursorCellInput {
  screen: Rect
  surface: Rect
  cols: number
  rows: number
  cursorCol: number
  /** The cursor's row within the viewport; outside 0..rows when scrolled away. */
  cursorRow: number
}

/** Measures the cell after the cursor, or null when it is not on screen. */
export function cursorCellBox(input: CursorCellInput): CursorCellBox | null {
  const { screen, surface, cols, rows, cursorCol, cursorRow } = input
  if (cols <= 0 || rows <= 0 || cursorRow < 0 || cursorRow >= rows) return null
  const cellWidth = screen.width / cols
  const cellHeight = screen.height / rows
  if (!(cellWidth > 0 && cellHeight > 0)) return null
  const col = Math.min(cursorCol + 1, cols)
  return {
    left: screen.left - surface.left + col * cellWidth,
    top: screen.top - surface.top + cursorRow * cellHeight,
    cellWidth,
    cellHeight,
    cellsLeft: cols - col,
  }
}

/** The cell after the cursor, with the font and color the terminal draws in. */
export interface CursorCellView extends CursorCellBox {
  fontFamily?: string
  fontSize?: number
  color?: string
}

function sameView(a: CursorCellView | null, b: CursorCellView | null): boolean {
  if (!a || !b) return a === b
  return (
    a.left === b.left &&
    a.top === b.top &&
    a.cellWidth === b.cellWidth &&
    a.cellHeight === b.cellHeight &&
    a.cellsLeft === b.cellsLeft &&
    a.fontFamily === b.fontFamily &&
    a.fontSize === b.fontSize &&
    a.color === b.color
  )
}

/**
 * Follows the cell after the terminal cursor while mounted, through renders,
 * scrolling, resizes and font changes. The rendered screen's size gives the cell size, so
 * no xterm internals are read.
 */
export function useCursorCellBox(
  termRef: RefObject<Terminal | null>,
  surfaceRef: RefObject<HTMLElement | null>
): CursorCellView | null {
  const [view, setView] = useState<CursorCellView | null>(null)

  useLayoutEffect(() => {
    const term = termRef.current
    const surface = surfaceRef.current
    if (!term || !surface) return
    const update = () => {
      const screen = term.element?.querySelector<HTMLElement>(".xterm-screen")
      const buffer = term.buffer.active
      const box = screen
        ? cursorCellBox({
            screen: screen.getBoundingClientRect(),
            surface: surface.getBoundingClientRect(),
            cols: term.cols,
            rows: term.rows,
            cursorCol: buffer.cursorX,
            cursorRow: buffer.baseY + buffer.cursorY - buffer.viewportY,
          })
        : null
      const next = box && {
        ...box,
        fontFamily: term.options.fontFamily,
        fontSize: term.options.fontSize,
        color: term.options.theme?.foreground,
      }
      setView((current) => (sameView(current, next) ? current : next))
    }
    update()
    const disposables = [term.onRender(update), term.onResize(update), term.onScroll(update)]
    const observer = new ResizeObserver(update)
    observer.observe(surface)
    return () => {
      disposables.forEach((disposable) => disposable.dispose())
      observer.disconnect()
    }
  }, [termRef, surfaceRef])

  return view
}
