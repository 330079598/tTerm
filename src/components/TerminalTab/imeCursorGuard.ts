import type { IDisposable, Terminal } from "@xterm/xterm"

/**
 * Keeps the IME candidate window from chasing the terminal cursor around.
 *
 * xterm.js places its hidden helper textarea (which the OS IME uses as the
 * caret) at the real terminal cursor, and while composing it re-reads that
 * cursor on every render. TUIs such as Claude Code and Codex hide the real
 * cursor, draw their own, and move the real one across the screen on every
 * redraw, so the candidate window jumps with spinners and status lines.
 *
 * The guard changes two things:
 *  1. A composition stays anchored to the cell it started at until it ends.
 *  2. While the cursor is hidden the textarea stops following it. A composition
 *     that starts during a brief hide (a redraw in flight) anchors to the last
 *     visible cursor position; after a long hide (the app draws its own cursor)
 *     no visible position is meaningful, so it anchors to the current cursor.
 *
 * It patches xterm 6 private members on the instance; if any are missing the
 * guard is a no-op and xterm keeps its default behaviour.
 */

/** A hide shorter than this is treated as a redraw in flight. */
export const TRANSIENT_CURSOR_HIDE_MS = 500

const DECTCEM = 25

interface CellPosition {
  col: number
  row: number
}

interface CompositionHelperInternals {
  readonly isComposing: boolean
  compositionstart: () => void
  updateCompositionElements: (dontRecurse?: boolean) => void
  _compositionView: HTMLElement
  _textarea: HTMLTextAreaElement
}

interface CoreInternals {
  cols: number
  rows: number
  buffer: { x: number; y: number; isCursorInViewport: boolean }
  coreService: { isCursorHidden: boolean }
  _renderService?: { dimensions: { css: { cell: { width: number; height: number } } } }
  _compositionHelper?: CompositionHelperInternals
  _syncTextArea?: () => void
}

const NOOP_DISPOSABLE: IDisposable = { dispose: () => {} }

function getCore(term: Terminal): CoreInternals | null {
  const core = (term as unknown as { _core?: CoreInternals })._core
  if (
    !core ||
    typeof core._syncTextArea !== "function" ||
    !core._compositionHelper ||
    typeof core._compositionHelper.compositionstart !== "function" ||
    typeof core._compositionHelper.updateCompositionElements !== "function" ||
    !core.coreService ||
    typeof core.coreService.isCursorHidden !== "boolean"
  ) {
    return null
  }
  return core
}

function includesCursorMode(params: (number | number[])[]): boolean {
  return params.some((param) => (Array.isArray(param) ? param[0] : param) === DECTCEM)
}

export function installImeCursorGuard(
  term: Terminal,
  now: () => number = () => performance.now()
): IDisposable {
  const core = getCore(term)
  if (!core) {
    console.warn("IME cursor guard: xterm internals not found; using default IME positioning")
    return NOOP_DISPOSABLE
  }
  const helper = core._compositionHelper!
  const originalSyncTextArea = core._syncTextArea!
  const originalCompositionStart = helper.compositionstart
  const originalUpdateCompositionElements = helper.updateCompositionElements

  let lastVisibleCursor: CellPosition | null = null
  let hiddenSince: number | null = core.coreService.isCursorHidden ? now() : null
  let compositionAnchor: CellPosition | null = null

  const currentCursor = (): CellPosition => ({
    col: Math.min(core.buffer.x, core.cols - 1),
    row: core.buffer.y,
  })

  const cellSize = () => core._renderService?.dimensions.css.cell

  const placeAt = (element: HTMLElement, anchor: CellPosition) => {
    const cell = cellSize()
    if (!cell) return null
    const col = Math.min(anchor.col, core.cols - 1)
    const row = Math.min(anchor.row, core.rows - 1)
    element.style.left = `${col * cell.width}px`
    element.style.top = `${row * cell.height}px`
    return cell
  }

  const syncTextArea = () => {
    if (core.coreService.isCursorHidden) return
    originalSyncTextArea.call(core)
    if (!helper.isComposing && core.buffer.isCursorInViewport) {
      lastVisibleCursor = currentCursor()
    }
  }

  const pickCompositionAnchor = (): CellPosition => {
    if (!core.coreService.isCursorHidden) return currentCursor()
    const hiddenFor = hiddenSince === null ? Infinity : now() - hiddenSince
    if (lastVisibleCursor && hiddenFor < TRANSIENT_CURSOR_HIDE_MS) return lastVisibleCursor
    return currentCursor()
  }

  const compositionStart = () => {
    compositionAnchor = pickCompositionAnchor()
    originalCompositionStart.call(helper)
    // The IME may read the caret rect before the first compositionupdate.
    placeAt(helper._textarea, compositionAnchor)
  }

  // Mirrors CompositionHelper.updateCompositionElements, but positions against
  // the anchor captured at compositionstart instead of the live cursor.
  const updateCompositionElements = (dontRecurse?: boolean) => {
    if (!helper.isComposing) {
      compositionAnchor = null
      return
    }
    const anchor = compositionAnchor
    if (!anchor) {
      originalUpdateCompositionElements.call(helper, dontRecurse)
      return
    }

    const view = helper._compositionView
    const cell = placeAt(view, anchor)
    if (cell) {
      view.style.height = `${cell.height}px`
      view.style.lineHeight = `${cell.height}px`
      view.style.fontFamily = term.options.fontFamily ?? ""
      view.style.fontSize = `${term.options.fontSize}px`

      const textarea = helper._textarea
      const viewBounds = view.getBoundingClientRect()
      textarea.style.left = view.style.left
      textarea.style.top = view.style.top
      // Ensure the text area is at least 1x1, otherwise certain IMEs may break
      textarea.style.width = `${Math.max(viewBounds.width, 1)}px`
      textarea.style.height = `${Math.max(viewBounds.height, 1)}px`
      textarea.style.lineHeight = `${viewBounds.height}px`
    }

    if (!dontRecurse) {
      setTimeout(() => updateCompositionElements(true), 0)
    }
  }

  core._syncTextArea = syncTextArea
  helper.compositionstart = compositionStart
  helper.updateCompositionElements = updateCompositionElements

  // Returning false lets xterm's own DECSET/DECRST handling still run.
  const cursorModeHandlers = [
    term.parser.registerCsiHandler({ prefix: "?", final: "l" }, (params) => {
      // Runs before xterm applies the mode, so this reads the previous state.
      if (includesCursorMode(params) && !core.coreService.isCursorHidden) {
        hiddenSince = now()
      }
      return false
    }),
    term.parser.registerCsiHandler({ prefix: "?", final: "h" }, (params) => {
      if (includesCursorMode(params)) {
        hiddenSince = null
        // Showing the cursor without moving it fires no cursor-move event, so
        // re-sync once xterm has applied the mode change.
        queueMicrotask(syncTextArea)
      }
      return false
    }),
  ]

  return {
    dispose: () => {
      for (const handler of cursorModeHandlers) handler.dispose()
      if (core._syncTextArea === syncTextArea) core._syncTextArea = originalSyncTextArea
      if (helper.compositionstart === compositionStart) {
        helper.compositionstart = originalCompositionStart
      }
      if (helper.updateCompositionElements === updateCompositionElements) {
        helper.updateCompositionElements = originalUpdateCompositionElements
      }
    },
  }
}
