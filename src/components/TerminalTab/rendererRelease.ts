import type { Terminal } from "@xterm/xterm"

/**
 * Disposes a renderer addon and frees the canvases it drew on right away.
 * The canvas renderer keeps four full-size layers per terminal and the WebGL
 * renderer a drawing buffer; disposing only removes them from the page, and
 * WebKit keeps their backing stores and GL context until a garbage collection
 * it may put off for a long time. A 2d canvas sized to nothing drops its
 * backing store; a WebGL canvas needs more, see releaseWebglCanvas.
 */
export function disposeRendererAddon(term: Terminal, addon: { dispose: () => void }): void {
  const screen = term.element?.querySelector(".xterm-screen")
  const canvases = screen ? Array.from(screen.querySelectorAll("canvas")) : []
  addon.dispose()
  for (const canvas of canvases) {
    if (screen?.contains(canvas)) continue
    // A canvas holds one kind of context, so asking for 2d tells them apart
    // without creating a GL context on a 2d canvas.
    if (canvas.getContext("2d")) {
      canvas.width = 0
      canvas.height = 0
      continue
    }
    const gl = canvas.getContext("webgl2")
    if (gl && !gl.isContextLost()) {
      releaseWebglCanvas(canvas, gl)
    } else {
      canvas.width = 0
      canvas.height = 0
    }
  }
}

const RELEASE_LOT_ATTRIBUTE = "data-tterm-webgl-release"

/**
 * Frees a removed WebGL canvas's buffers in the GPU process. WebKit skips
 * resizing the drawing buffer of a lost context, and keeps the buffer last
 * shown until a later frame replaces it, so losing the context or sizing the
 * canvas down after removal frees nothing until the context is collected.
 * Back in the page out of sight, sized to one pixel and cleared, the canvas
 * shows a one-pixel frame at the next rendering update, which lets go of the
 * full-size buffers; only then is it removed and its context lost. Frames
 * stop while the window is hidden, so the release finishes once it is shown.
 */
function releaseWebglCanvas(canvas: HTMLCanvasElement, gl: WebGL2RenderingContext): void {
  let lot = document.querySelector<HTMLElement>(`[${RELEASE_LOT_ATTRIBUTE}]`)
  if (!lot) {
    lot = document.createElement("div")
    lot.setAttribute(RELEASE_LOT_ATTRIBUTE, "")
    lot.setAttribute("aria-hidden", "true")
    lot.style.cssText =
      "position:fixed;left:0;top:0;width:1px;height:1px;overflow:hidden;visibility:hidden;pointer-events:none"
    document.body.appendChild(lot)
  }
  canvas.style.width = "1px"
  canvas.style.height = "1px"
  lot.appendChild(canvas)
  canvas.width = 1
  canvas.height = 1
  gl.bindFramebuffer(gl.FRAMEBUFFER, null)
  gl.disable(gl.SCISSOR_TEST)
  gl.viewport(0, 0, 1, 1)
  gl.colorMask(true, true, true, true)
  gl.clearColor(0, 0, 0, 0)
  gl.clear(gl.COLOR_BUFFER_BIT)

  const finish = () => {
    const parent = canvas.parentElement
    canvas.remove()
    gl.getExtension("WEBGL_lose_context")?.loseContext()
    canvas.width = 0
    canvas.height = 0
    if (parent && !parent.hasChildNodes()) parent.remove()
  }
  // The first frame shows the one-pixel buffer, the second runs after it.
  requestAnimationFrame(() => requestAnimationFrame(finish))
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
