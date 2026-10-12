import { useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import type { TerminalTabProps } from "@/components/TerminalTab/types"

const FIT_STABILITY_FRAMES = 2
const MAX_PENDING_FIT_FRAMES = 4

type UseTerminalFitOptions = Pick<
  TerminalTabRefs,
  | "fitAddonRef"
  | "termRef"
  | "lastPtySizeRef"
  | "resizeRafRef"
  | "resizePtySyncTimerRef"
  | "isActiveRef"
> &
  Pick<TerminalTabProps, "tabId"> & { sessionNonce: number }

/**
 * Fits the terminal to its pane and tells the PTY the new size, settling
 * over a few frames while a resize is under way.
 */
export function useTerminalFit({
  fitAddonRef,
  termRef,
  lastPtySizeRef,
  resizeRafRef,
  resizePtySyncTimerRef,
  isActiveRef,
  tabId,
  sessionNonce,
}: UseTerminalFitOptions) {
  const fitTerminalOnly = useCallback(() => {
    const fitAddon = fitAddonRef.current
    const term = termRef.current
    if (!fitAddon || !term) return false

    const dimensions = fitAddon.proposeDimensions()
    if (!dimensions) return false

    fitAddon.fit()
    return term.cols === dimensions.cols && term.rows === dimensions.rows
  }, [fitAddonRef, termRef])

  const syncPtySize = useCallback(
    (force = false) => {
      const term = termRef.current
      if (!term) return

      const nextSize = { rows: term.rows, cols: term.cols }
      const prevSize = lastPtySizeRef.current
      // No PTY yet: create_pty sends the size itself, and catches up after.
      if (!prevSize) return
      if (!force && prevSize.rows === nextSize.rows && prevSize.cols === nextSize.cols) {
        return
      }

      lastPtySizeRef.current = nextSize
      invoke("resize_pty", {
        tabId,
        sessionNonce,
        rows: nextSize.rows,
        cols: nextSize.cols,
      }).catch(console.error)
    },
    [lastPtySizeRef, sessionNonce, tabId, termRef]
  )

  const fitAndSyncPty = useCallback(
    (force = false) => {
      fitTerminalOnly()
      syncPtySize(force)
    },
    [fitTerminalOnly, syncPtySize]
  )

  const scheduleFitDuringResize = useCallback(() => {
    if (resizeRafRef.current !== null) {
      window.cancelAnimationFrame(resizeRafRef.current)
    }
    if (resizePtySyncTimerRef.current !== null) {
      window.clearTimeout(resizePtySyncTimerRef.current)
      resizePtySyncTimerRef.current = null
    }

    let frameCount = 0
    let stableFrameCount = 0
    const fitAfterLayout = () => {
      resizeRafRef.current = window.requestAnimationFrame(() => {
        resizeRafRef.current = null
        if (!isActiveRef.current) return

        frameCount += 1
        const fitted = fitTerminalOnly()
        stableFrameCount = fitted ? stableFrameCount + 1 : 0

        if (stableFrameCount < FIT_STABILITY_FRAMES && frameCount < MAX_PENDING_FIT_FRAMES) {
          fitAfterLayout()
          return
        }

        if (!fitted) return

        const term = termRef.current
        if (term && term.rows > 0) {
          term.refresh(0, term.rows - 1)
        }

        resizePtySyncTimerRef.current = window.setTimeout(() => {
          resizePtySyncTimerRef.current = null
          if (isActiveRef.current) {
            syncPtySize()
          }
        }, 80)
      })
    }

    fitAfterLayout()
  }, [fitTerminalOnly, isActiveRef, resizePtySyncTimerRef, resizeRafRef, syncPtySize, termRef])

  return {
    fitTerminalOnly,
    fitAndSyncPty,
    scheduleFitDuringResize,
  }
}
