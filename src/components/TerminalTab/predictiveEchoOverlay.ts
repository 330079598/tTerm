import type { Terminal } from "@xterm/xterm"

import type { PredictionView } from "@/lib/predictiveEcho"

/** The cell the terminal's own cursor is in, while the predicted cursor is elsewhere. */
export interface RealCursor {
  col: number
  /** Its cell is empty, so covering it hides the cursor and nothing else. */
  empty: boolean
}

/**
 * Covers a cell. A transparent background (under the window blur) would let
 * the cell show through, so its color is taken opaque.
 */
function coverColor(background: string | undefined): string {
  if (!background) return "#000000"
  return /^#[\da-f]{8}$/i.test(background) ? background.slice(0, 7) : background
}

function cellStyle(left: number, top: number, width: number, height: number): string {
  return `position:absolute;left:${left}px;top:${top}px;width:${width}px;height:${height}px;`
}

/**
 * Draws the predicted echo over the terminal: each predicted cell on a
 * background that hides what is under it, and the cursor where the echo
 * will leave it. None of it is terminal content.
 */
export function createPredictiveEchoOverlay(
  term: Terminal,
  charAt: (row: number, col: number) => string
) {
  const layer = document.createElement("div")
  layer.className = "terminal-predictive-echo"
  layer.setAttribute("aria-hidden", "true")
  layer.style.cssText =
    "position:absolute;inset:0;pointer-events:none;z-index:7;overflow:hidden;white-space:pre;text-align:center;"
  let drawn = false

  const clear = () => {
    if (!drawn) return
    layer.replaceChildren()
    drawn = false
  }

  const draw = (view: PredictionView | null, realCursor: RealCursor) => {
    const screen = term.element?.querySelector<HTMLElement>(".xterm-screen")
    const row = view ? view.row - term.buffer.active.viewportY : -1
    if (!view || !screen || row < 0 || row >= term.rows) {
      clear()
      return
    }
    if (layer.parentElement !== screen) screen.appendChild(layer)
    // xterm sizes the screen to whole cells; the inline size is unrounded and
    // not scaled by the interface zoom.
    const width = (parseFloat(screen.style.width) || screen.offsetWidth) / term.cols
    const height = (parseFloat(screen.style.height) || screen.offsetHeight) / term.rows
    const top = row * height
    const theme = term.options.theme ?? {}
    const background = coverColor(theme.background)
    layer.style.fontFamily = term.options.fontFamily ?? ""
    layer.style.fontSize = `${term.options.fontSize ?? 14}px`
    layer.style.fontWeight = String(term.options.fontWeight ?? "normal")
    layer.style.lineHeight = `${height}px`
    layer.style.color = theme.foreground ?? "#ffffff"

    const nodes: HTMLElement[] = []
    const covered = new Set(view.cells.map((cell) => cell.col))
    for (const cell of view.cells) {
      const node = document.createElement("span")
      node.style.cssText = `${cellStyle(cell.col * width, top, width, height)}background:${background};`
      if (cell.stalled) node.style.textDecoration = "underline"
      node.textContent = cell.char
      nodes.push(node)
    }
    if (realCursor.empty && realCursor.col !== view.cursorCol && !covered.has(realCursor.col)) {
      const node = document.createElement("span")
      node.style.cssText = `${cellStyle(realCursor.col * width, top, width, height)}background:${background};`
      nodes.push(node)
    }
    nodes.push(drawCursor(view, width, height, top, background))
    layer.replaceChildren(...nodes)
    drawn = true
  }

  const drawCursor = (
    view: PredictionView,
    width: number,
    height: number,
    top: number,
    background: string
  ) => {
    const theme = term.options.theme ?? {}
    const color = theme.cursor ?? theme.foreground ?? "#ffffff"
    const cursor = document.createElement("span")
    const left = view.cursorCol * width
    const style = term.options.cursorStyle ?? "block"
    if (style === "bar") {
      const bar = Math.max(1, term.options.cursorWidth ?? 1)
      cursor.style.cssText = `${cellStyle(left, top, bar, height)}background:${color};`
    } else if (style === "underline") {
      cursor.style.cssText = `${cellStyle(left, top + height - 2, width, 2)}background:${color};`
    } else {
      cursor.style.cssText = `${cellStyle(left, top, width, height)}background:${color};color:${theme.cursorAccent ?? background};`
      cursor.textContent =
        view.cells.find((cell) => cell.col === view.cursorCol)?.char ??
        charAt(view.row, view.cursorCol)
    }
    return cursor
  }

  return {
    draw,
    dispose: () => layer.remove(),
  }
}
