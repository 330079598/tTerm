import type { Terminal } from "@xterm/xterm"

/**
 * Disposes a renderer addon and frees the canvases it drew on right away.
 * The canvas renderer keeps four full-size layers per terminal and the WebGL
 * renderer a drawing buffer; disposing only removes them from the page, and
 * WebKit keeps their backing stores and GL context until a garbage collection
 * it may put off for a long time. A canvas sized to nothing drops its backing
 * store, and losing the context frees the GL resources and the slot WebKit
 * caps the live contexts of a page at.
 */
export function disposeRendererAddon(term: Terminal, addon: { dispose: () => void }): void {
  const screen = term.element?.querySelector(".xterm-screen")
  const canvases = screen ? Array.from(screen.querySelectorAll("canvas")) : []
  addon.dispose()
  for (const canvas of canvases) {
    if (screen?.contains(canvas)) continue
    try {
      const gl = canvas.getContext("webgl2")
      gl?.getExtension("WEBGL_lose_context")?.loseContext()
    } catch {
      // A 2d canvas has no WebGL context to lose
    }
    canvas.width = 0
    canvas.height = 0
  }
}

type RenderServiceInternals = {
  _core?: { _renderService?: { _renderRows?: (start: number, end: number) => void } }
}

/**
 * Draws every row now. A terminal coming back into sight is still paused
 * until xterm's intersection observer reports it visible, which only happens
 * after the first frame is painted, so a renderer loaded for it would show
 * that frame empty; the canvas the released renderer drew on is gone.
 */
export function renderAllRowsNow(term: Terminal): void {
  const renderService = (term as unknown as RenderServiceInternals)._core?._renderService
  const end = Math.max(0, term.rows - 1)
  if (typeof renderService?._renderRows === "function") {
    renderService._renderRows(0, end)
  } else {
    term.refresh(0, end)
  }
}
