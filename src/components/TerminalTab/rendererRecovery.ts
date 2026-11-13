import type { Terminal } from "@xterm/xterm"

interface RenderServiceInternals {
  _core?: {
    _renderService?: {
      clear?: () => void
      _renderRows?: (start: number, end: number) => void
      _needsSelectionRefresh?: boolean
      _renderer?: {
        value?: {
          _glyphRenderer?: { value?: { _atlasTextures?: { version: number }[] } }
        }
      }
    }
  }
}

/**
 * Draws every row now. A terminal coming back into sight is still paused
 * until xterm's intersection observer reports it visible, which only happens
 * after the first frame is painted, so a renderer loaded for it would show
 * that frame empty; the canvas the released renderer drew on is gone.
 */
export function renderAllRowsNow(term: Terminal): void {
  const service = (term as unknown as RenderServiceInternals)._core?._renderService
  const end = Math.max(0, term.rows - 1)
  if (typeof service?._renderRows === "function") {
    service._renderRows(0, end)
  } else {
    term.refresh(0, end)
  }
}

/**
 * Rebuilds the displayed frame on activation or resume, without changing the
 * terminal buffer, scroll position, selection or PTY. A plain refresh skips
 * unchanged cells and atlas versions; invalidate those local renderer caches
 * so a stale GPU texture cannot survive it. Do not clear the shared atlas:
 * addon-webgl 0.19 cannot safely merge pages emptied by clearTextureAtlas.
 */
export function recoverTerminalRenderer(term: Terminal): void {
  const service = (term as unknown as RenderServiceInternals)._core?._renderService
  const textures = service?._renderer?.value?._glyphRenderer?.value?._atlasTextures
  if (Array.isArray(textures)) {
    for (const texture of textures) texture.version = -1
  }
  // This is RenderService.clear, not Terminal.clear (which deletes history).
  service?.clear?.()
  // Canvas.clear also clears its selection layer; let RenderService replay
  // the existing selection, including column selection, after drawing rows.
  if (service) service._needsSelectionRefresh = true
  renderAllRowsNow(term)
}
