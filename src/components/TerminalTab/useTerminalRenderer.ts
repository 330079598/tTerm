import { useCallback, useEffect, useLayoutEffect, useRef } from "react"
import { CanvasAddon } from "@xterm/addon-canvas"
import { WebglAddon } from "@xterm/addon-webgl"
import { Terminal } from "@xterm/xterm"

import type { UseTerminalLifecycleOptions } from "@/components/TerminalTab/terminalLifecycleTypes"
import { disposeRendererAddon, renderAllRowsNow } from "@/components/TerminalTab/rendererRelease"
import type { TerminalRenderer } from "@/lib/appConfig"

type ActiveRendererAddon = (WebglAddon | CanvasAddon) & {
  dispose: () => void
  clearTextureAtlas?: () => void
}
/** Wait after a lost WebGL context before trying WebGL again. */
const WEBGL_RETRY_DELAY_MS = 30_000
/** WebGL retries per terminal after lost contexts. */
const MAX_WEBGL_RETRIES = 3

/**
 * The terminal's renderer: loading WebGL (falling back to canvas, and
 * retrying WebGL after a lost context) or canvas.
 */
export function useTerminalRenderer(termRef: UseTerminalLifecycleOptions["termRef"]) {
  const activeRendererAddonRef = useRef<ActiveRendererAddon | null>(null)
  const lastRendererRef = useRef<TerminalRenderer | null>(null)
  // Out of sight long enough to have its renderer released; showing the
  // terminal again loads it back.
  const rendererSuspendedRef = useRef(false)
  const webglRetriesRef = useRef(0)
  const webglRetryTimerRef = useRef<number | null>(null)

  const loadTerminalRenderer = useCallback(
    (targetRenderer: TerminalRenderer, targetTerm?: Terminal) => {
      const term = targetTerm ?? termRef.current
      if (!term) return

      if (activeRendererAddonRef.current) {
        try {
          disposeRendererAddon(term, activeRendererAddonRef.current)
        } catch (error) {
          console.error("Failed to dispose active terminal renderer addon:", error)
        }
        activeRendererAddonRef.current = null
      }

      if (targetRenderer === "webgl") {
        const loadWebgl = (): boolean => {
          try {
            const webglAddon = new WebglAddon()
            webglAddon.onContextLoss(() => {
              console.warn("WebGL context lost; falling back to canvas renderer")
              try {
                webglAddon.dispose()
              } catch (disposeErr) {
                console.warn("Failed to dispose WebGL addon on context loss:", disposeErr)
              }
              if (activeRendererAddonRef.current === webglAddon) {
                activeRendererAddonRef.current = null
              }
              let canvasAddon: CanvasAddon
              try {
                canvasAddon = new CanvasAddon()
                term.loadAddon(canvasAddon)
                activeRendererAddonRef.current = canvasAddon
                lastRendererRef.current = "canvas"
              } catch (canvasErr) {
                console.error("Failed to load canvas fallback after WebGL context loss:", canvasErr)
                return
              }
              // The canvas fallback keeps four full-size layers, so go back to
              // WebGL once the GPU may have recovered (a GPU switch or reset).
              // A terminal released while hidden gets WebGL back when shown;
              // the retries are capped so a page over WebKit's context limit
              // does not keep evicting other terminals' contexts.
              if (webglRetriesRef.current >= MAX_WEBGL_RETRIES) return
              webglRetriesRef.current += 1
              webglRetryTimerRef.current = window.setTimeout(() => {
                webglRetryTimerRef.current = null
                if (activeRendererAddonRef.current !== canvasAddon) return
                try {
                  disposeRendererAddon(term, canvasAddon)
                } catch (disposeErr) {
                  console.warn("Failed to dispose canvas fallback before WebGL retry:", disposeErr)
                }
                activeRendererAddonRef.current = null
                lastRendererRef.current = null
                if (!loadWebgl()) {
                  try {
                    const fallback = new CanvasAddon()
                    term.loadAddon(fallback)
                    activeRendererAddonRef.current = fallback
                    lastRendererRef.current = "canvas"
                  } catch (canvasErr) {
                    console.error("Failed to reload canvas renderer after WebGL retry:", canvasErr)
                  }
                }
                renderAllRowsNow(term)
              }, WEBGL_RETRY_DELAY_MS)
            })
            term.loadAddon(webglAddon)
            activeRendererAddonRef.current = webglAddon
            lastRendererRef.current = "webgl"
            return true
          } catch (error) {
            console.warn(
              "WebGL not supported in this environment; falling back to canvas renderer",
              error
            )
            return false
          }
        }
        if (loadWebgl()) return
      }

      // "canvas" mode or fallback from failed WebGL
      try {
        const canvasAddon = new CanvasAddon()
        term.loadAddon(canvasAddon)
        activeRendererAddonRef.current = canvasAddon
        lastRendererRef.current = "canvas"
      } catch (error) {
        console.error("Failed to load canvas renderer; using DOM renderer fallback", error)
        lastRendererRef.current = null
      }
    },
    [termRef]
  )

  const clearTextureAtlas = useCallback(() => {
    activeRendererAddonRef.current?.clearTextureAtlas?.()
  }, [])
  return {
    activeRendererAddonRef,
    lastRendererRef,
    rendererSuspendedRef,
    webglRetryTimerRef,
    loadTerminalRenderer,
    clearTextureAtlas,
  }
}

type TerminalRendererSyncOptions = Pick<
  UseTerminalLifecycleOptions,
  | "termRef"
  | "initializedRef"
  | "terminalRenderer"
  | "isActiveRef"
  | "fitTerminalOnly"
  | "isActive"
  | "hiddenRendererReleaseSecs"
  | "sessionNonce"
> &
  Pick<
    ReturnType<typeof useTerminalRenderer>,
    "activeRendererAddonRef" | "lastRendererRef" | "rendererSuspendedRef" | "loadTerminalRenderer"
  > & {
    rendererRef: React.RefObject<TerminalRenderer>
  }

/**
 * Follows the renderer setting, and releases the renderer of a terminal out
 * of sight long enough, loading it back once the terminal is shown.
 */
export function useTerminalRendererSync({
  termRef,
  initializedRef,
  terminalRenderer,
  isActiveRef,
  fitTerminalOnly,
  isActive,
  hiddenRendererReleaseSecs,
  sessionNonce,
  activeRendererAddonRef,
  lastRendererRef,
  rendererSuspendedRef,
  loadTerminalRenderer,
  rendererRef,
}: TerminalRendererSyncOptions) {
  useEffect(() => {
    const term = termRef.current
    if (!term || !initializedRef.current) return
    const targetRenderer = terminalRenderer ?? rendererRef.current
    if (rendererSuspendedRef.current || lastRendererRef.current === targetRenderer) return

    loadTerminalRenderer(targetRenderer, term)
    if (isActiveRef.current) {
      fitTerminalOnly()
      term.refresh(0, Math.max(0, term.rows - 1))
    }
  }, [
    fitTerminalOnly,
    initializedRef,
    isActiveRef,
    lastRendererRef,
    loadTerminalRenderer,
    rendererRef,
    rendererSuspendedRef,
    termRef,
    terminalRenderer,
  ])

  // Before paint, so a terminal coming into sight never shows a frame without
  // its renderer.
  useLayoutEffect(() => {
    const term = termRef.current
    if (!term || !initializedRef.current) return

    if (isActive) {
      if (!rendererSuspendedRef.current) return
      rendererSuspendedRef.current = false
      loadTerminalRenderer(terminalRenderer ?? rendererRef.current, term)
      renderAllRowsNow(term)
      return
    }
    if (hiddenRendererReleaseSecs <= 0) return

    const timer = window.setTimeout(() => {
      const addon = activeRendererAddonRef.current
      if (!addon || termRef.current !== term) return
      try {
        disposeRendererAddon(term, addon)
      } catch (error) {
        console.warn("Failed to release the renderer of a hidden terminal:", error)
      }
      activeRendererAddonRef.current = null
      lastRendererRef.current = null
      rendererSuspendedRef.current = true
    }, hiddenRendererReleaseSecs * 1000)
    return () => window.clearTimeout(timer)
  }, [
    activeRendererAddonRef,
    hiddenRendererReleaseSecs,
    initializedRef,
    isActive,
    lastRendererRef,
    loadTerminalRenderer,
    rendererRef,
    rendererSuspendedRef,
    sessionNonce,
    termRef,
    terminalRenderer,
  ])
}
