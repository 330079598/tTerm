/**
 * Canvas Font Host
 *
 * In WebKit (macOS Safari / Tauri WKWebView), any <canvas> element that is detached
 * from the active document DOM tree is subject to font fingerprinting protections:
 * WebKit refuses to resolve user-installed local fonts (like those in ~/Library/Fonts)
 * on detached canvases, silently falling back to generic monospace ("Courier").
 *
 * xterm.js creates an internal scratch canvas (TextureAtlas._tmpCanvas) detached from
 * the DOM and performs warmUp() on it, rasterizing all standard ASCII glyphs into
 * the texture cache using Courier. When text is later selected with the mouse,
 * xterm appends _tmpCanvas to terminal.element to inherit styles, which suddenly
 * allows WebKit to resolve the user's custom font, causing selected text to render
 * in the custom font while unselected text remains in Courier.
 *
 * This utility ensures that any newly created <canvas> is temporarily attached to
 * an invisible, non-interactive offscreen host container in document.body.
 *
 * Protections & Resource Safeguards:
 * 1. Bounded capacity (LRU): At most MAX_HOST_CANVASES (512) are kept in the host.
 *    Excess/orphaned scratch canvases are evicted and released for garbage collection.
 * 2. Pinned xterm canvases: Canvases created by xterm TextureAtlas/CharAtlas are pinned
 *    and immune to normal LRU eviction so that long-lived terminal sessions never lose font protection.
 * 3. Activity refreshing: Calling getContext("2d") marks the canvas and refreshes its LRU position.
 * 4. WebGL Context decoupling: Requesting a "webgl" / "webgl2" / "bitmaprenderer" context
 *    immediately detaches the canvas from the host to prevent WKWebView visibility throttling.
 * 5. Automatic reparenting: When xterm or any other component calls `parent.appendChild(canvas)`,
 *    the browser automatically moves it to the target container.
 */

export const HOST_ELEMENT_ID = "tterm-canvas-font-host"
export const MAX_HOST_CANVASES = 512

let originalCreateElement: typeof document.createElement | null = null
let originalGetContext: typeof HTMLCanvasElement.prototype.getContext | null = null
let hostElement: HTMLElement | null = null

function getOrCreateHostElement(): HTMLElement | null {
  if (typeof document === "undefined") {
    return null
  }

  if (hostElement && hostElement.isConnected) {
    return hostElement
  }

  const existing = document.getElementById(HOST_ELEMENT_ID)
  if (existing) {
    hostElement = existing
    return hostElement
  }

  if (!document.body) {
    return null
  }

  const host = document.createElement("div")
  host.id = HOST_ELEMENT_ID
  host.setAttribute("aria-hidden", "true")
  host.style.cssText =
    "position:fixed;top:-99999px;left:-99999px;width:0;height:0;overflow:hidden;visibility:hidden;pointer-events:none;opacity:0;"

  document.body.appendChild(host)
  hostElement = host
  return hostElement
}

/**
 * Appends a canvas to the host element, maintaining a bounded capacity (LRU eviction).
 * Prioritizes retaining pinned xterm canvases and active 2D contexts.
 */
function attachCanvasToHost(canvas: HTMLCanvasElement): void {
  try {
    const host = getOrCreateHostElement()
    if (!host || !host.isConnected) {
      return
    }

    // Evict unreferenced/temporary canvases if capacity is exceeded
    while (host.childElementCount >= MAX_HOST_CANVASES) {
      let candidateToEvict: Element | null = null

      for (let i = 0; i < host.children.length; i++) {
        const child = host.children[i]
        // Never prioritize evicting pinned xterm canvases
        if (child.getAttribute?.("data-pinned-font-host") === "true") {
          continue
        }

        if (!candidateToEvict) {
          candidateToEvict = child
        } else if (
          child.getAttribute?.("data-has-2d-context") !== "true" &&
          candidateToEvict.getAttribute?.("data-has-2d-context") === "true"
        ) {
          candidateToEvict = child
        }
      }

      // If all canvases are pinned or no unpinned canvas found, fallback to firstElementChild
      const target = candidateToEvict || host.firstElementChild
      if (target) {
        if (target.getAttribute?.("data-pinned-font-host") === "true") {
          console.warn(
            "[canvasFontHost] Evicting a pinned xterm canvas due to host capacity saturation",
            target
          )
        }
        host.removeChild(target)
      } else {
        break
      }
    }

    host.appendChild(canvas)
  } catch {
    // Silently ignore attachment errors in mock or restricted DOM environments
  }
}

/**
 * Initializes the Canvas Font Host interceptor and context guards.
 * Safe to call multiple times (idempotent).
 */
export function initCanvasFontHost(): void {
  if (typeof document === "undefined" || originalCreateElement) {
    return
  }

  // Ensure host element is appended as soon as body is available
  if (document.body) {
    getOrCreateHostElement()
  } else {
    document.addEventListener(
      "DOMContentLoaded",
      () => {
        getOrCreateHostElement()
      },
      { once: true }
    )
  }

  originalCreateElement = document.createElement.bind(document)

  document.createElement = function <K extends keyof HTMLElementTagNameMap>(
    tagName: K,
    options?: ElementCreationOptions
  ): HTMLElementTagNameMap[K] {
    if (!originalCreateElement) {
      return document.createElement(tagName, options)
    }

    const element = originalCreateElement(tagName, options)

    if (typeof tagName === "string" && tagName.toLowerCase() === "canvas") {
      const canvas = element as unknown as HTMLCanvasElement
      try {
        const stack = new Error().stack || ""
        if (
          stack.includes("TextureAtlas") ||
          stack.includes("CharAtlas") ||
          stack.includes("xterm")
        ) {
          canvas.setAttribute("data-pinned-font-host", "true")
        }
      } catch {
        // Silently ignore stack inspection errors
      }

      attachCanvasToHost(canvas)
    }

    return element
  } as typeof document.createElement

  // Intercept getContext to decouple WebGL from hidden host and refresh 2D LRU
  if (typeof HTMLCanvasElement !== "undefined" && HTMLCanvasElement.prototype) {
    originalGetContext = HTMLCanvasElement.prototype.getContext

    HTMLCanvasElement.prototype.getContext = function (
      this: HTMLCanvasElement,
      contextId: string,
      ...args: unknown[]
    ): RenderingContext | null {
      if (hostElement && this.parentElement === hostElement) {
        if (contextId === "webgl" || contextId === "webgl2" || contextId === "bitmaprenderer") {
          // WebGL / bitmap contexts do not need 2D font rasterization protections.
          // Detach immediately to prevent WKWebView background/visibility throttling.
          try {
            hostElement.removeChild(this)
          } catch {
            // Ignore if already detached
          }
        } else if (contextId === "2d") {
          try {
            this.setAttribute("data-has-2d-context", "true")
            // Refresh LRU position by re-appending to the end of host children
            hostElement.appendChild(this)
          } catch {
            // Ignore attachment errors
          }
        }
      }

      if (!originalGetContext) {
        throw new Error(
          "canvasFontHost: original HTMLCanvasElement.prototype.getContext is unavailable"
        )
      }
      return Reflect.apply(originalGetContext, this, [
        contextId,
        ...args,
      ]) as RenderingContext | null
    } as typeof HTMLCanvasElement.prototype.getContext
  }
}

/**
 * Restores the original document.createElement and getContext methods.
 * Primarily used in test suites for cleanup.
 */
export function restoreCanvasFontHost(): void {
  if (typeof document !== "undefined" && originalCreateElement) {
    document.createElement = originalCreateElement
    originalCreateElement = null
  }

  if (
    typeof HTMLCanvasElement !== "undefined" &&
    HTMLCanvasElement.prototype &&
    originalGetContext
  ) {
    HTMLCanvasElement.prototype.getContext = originalGetContext
    originalGetContext = null
  }

  if (hostElement) {
    if (hostElement.parentElement) {
      hostElement.parentElement.removeChild(hostElement)
    }
    hostElement = null
  }
}

/**
 * Safely preloads a font using CSS font shorthand via document.fonts.load.
 * Performs input sanitization, ignores malformed font families, and never rejects.
 *
 * NOTE: document.fonts.load resolves with an empty array ([]) for local OS-installed
 * fonts (which are not in the FontFaceSet). Thus, resolving with any array (even empty)
 * indicates that the font specification was syntactically parsed by the engine.
 */
export async function safePreloadFont(fontSize: number, fontFamily: string): Promise<boolean> {
  if (typeof document === "undefined" || !document.fonts?.load) {
    return false
  }

  const trimmedFamily = (fontFamily || "").trim()
  const safeSize = Number.isFinite(fontSize) && fontSize > 0 ? fontSize : 14

  // Validate that fontFamily is non-empty and does not contain dangerous control characters
  if (!trimmedFamily || /[\n\r\t\0]/.test(trimmedFamily)) {
    return false
  }

  const fontSpec = `${safeSize}px ${trimmedFamily}`
  try {
    await document.fonts.load(fontSpec)
    return true
  } catch {
    // Silently swallow font syntax/loading errors
    return false
  }
}
