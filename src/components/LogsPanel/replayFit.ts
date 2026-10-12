import type { Terminal } from "@xterm/xterm"

import { MIN_REPLAY_FONT_SIZE, replayFontSize, type ReplayBox } from "@/lib/terminalLogReplay"

/** How far the font shrinks per step when pixel rounding leaves it too big. */
const SHRINK_STEP = 0.5

/**
 * Room kept for xterm's scrollbar (its default width), which sits at the
 * panel's right edge, so the last column is not drawn under it.
 */
const SCROLLBAR_WIDTH = 14

/** What the replay is asked to draw at. */
export interface ReplayFontRequest {
  /** The font size the user set. */
  preferred: number
  /** Use the largest size that fits the panel instead. */
  fit: boolean
}

export interface ReplayFontFit {
  fontSize: number
  /** The panel held the font below what was asked for. */
  capped: boolean
}

/** The parts of xterm's viewport that keep its scrollbar in step. */
interface ViewportInternals {
  _core?: {
    _viewport?: {
      _sync?: () => void
      scrollToLine?: (line: number, disableSmoothScroll?: boolean) => void
    }
  }
}

/**
 * Brings xterm's scrollbar up to date after the font changed. xterm measures
 * it only when the rows or the scroll position change, and its public scroll
 * calls work in pixels of the old row height, so a stale scrollbar is the
 * wrong height and scrolling lands on the wrong line. This measures it again
 * and puts `line` back at the top.
 */
function syncScrollbar(term: Terminal, line: number) {
  const viewport = (term as unknown as ViewportInternals)._core?._viewport
  viewport?._sync?.()
  viewport?.scrollToLine?.(line, true)
}

function boxOf(element: Element): ReplayBox {
  const rect = element.getBoundingClientRect()
  return { width: rect.width, height: rect.height }
}

function contentBoxOf(element: HTMLElement): ReplayBox {
  const style = getComputedStyle(element)
  return {
    width: element.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight),
    height: element.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom),
  }
}

/**
 * Sizes the replay's font as `request` asks, but small enough that the
 * recording fits `viewport`, and again whenever the panel or
 * the recorded size changes, reporting each size to `onFit`. The recording
 * keeps its own columns and rows, since replaying at another size would draw
 * its output differently from how it was drawn.
 */
export function fitReplayToViewport(
  term: Terminal,
  host: HTMLElement,
  viewport: HTMLElement,
  request: ReplayFontRequest,
  onFit: (fit: ReplayFontFit) => void
) {
  const fit = () => {
    const screen = host.querySelector(".xterm-screen")
    const available = contentBoxOf(viewport)
    if (!screen || available.width <= 0 || available.height <= 0) return
    const text = boxOf(screen)
    const chrome = { width: SCROLLBAR_WIDTH, height: 0 }
    const startSize = term.options.fontSize ?? request.preferred
    const topLine = term.buffer.active.viewportY
    const sized = replayFontSize(startSize, request.preferred, request.fit, text, chrome, available)
    let { fontSize } = sized
    if (fontSize !== term.options.fontSize) term.options.fontSize = fontSize
    // xterm rounds the cell to whole device pixels, which can still overflow.
    const overflows = () => {
      const box = boxOf(screen)
      return box.width + chrome.width > available.width || box.height > available.height
    }
    let capped = sized.capped
    while (fontSize > MIN_REPLAY_FONT_SIZE && overflows()) {
      fontSize = Math.max(MIN_REPLAY_FONT_SIZE, fontSize - SHRINK_STEP)
      term.options.fontSize = fontSize
      capped = true
    }
    if (fontSize !== startSize) syncScrollbar(term, topLine)
    onFit({ fontSize, capped: capped && !request.fit })
  }

  // Fitting resizes the panel's content, so it waits for the next frame
  // rather than changing layout inside the observer's own callback.
  let frame = 0
  const observer = new ResizeObserver(() => {
    cancelAnimationFrame(frame)
    frame = requestAnimationFrame(fit)
  })
  observer.observe(viewport)
  const resized = term.onResize(fit)
  fit()

  return {
    refit: fit,
    dispose: () => {
      cancelAnimationFrame(frame)
      observer.disconnect()
      resized.dispose()
    },
  }
}
