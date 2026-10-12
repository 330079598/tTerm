import { useEffect } from "react"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import type { useTerminalFit } from "@/components/TerminalTab/useTerminalFit"
import { TAB_ACTIVATE_REFIT_DELAY_MS } from "@/components/TerminalTab/terminalTabUtils"
import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { clearTabAttention } from "@/lib/tabAttention"
import { markActiveTerminal, registerTerminalFocus } from "@/lib/terminalFocus"
import { dismissTabToast } from "@/hooks/useTerminalNotifications"

type UseTerminalActivationOptions = Pick<
  TerminalTabProps,
  "workspacePanelApi" | "isActive" | "tabId" | "isGlobalShortcutTarget"
> &
  Pick<
    TerminalTabRefs,
    | "isActiveRef"
    | "containerRef"
    | "surfaceRef"
    | "resizeObserverRef"
    | "activateFitTimerRef"
    | "termRef"
    | "resizeRafRef"
    | "hasBeenActiveRef"
    | "shellRef"
    | "bellFlashTimerRef"
  > &
  Pick<ReturnType<typeof useTerminalFit>, "scheduleFitDuringResize">

/**
 * What happens as the tab comes into sight or leaves it: refitting,
 * focus, and clearing what it wanted attention for.
 */
export function useTerminalActivation({
  workspacePanelApi,
  isActive,
  tabId,
  isGlobalShortcutTarget,
  isActiveRef,
  containerRef,
  surfaceRef,
  resizeObserverRef,
  activateFitTimerRef,
  termRef,
  resizeRafRef,
  hasBeenActiveRef,
  shellRef,
  bellFlashTimerRef,
  scheduleFitDuringResize,
}: UseTerminalActivationOptions) {
  useEffect(() => {
    if (!workspacePanelApi) return

    const disposable = workspacePanelApi.onDidDimensionsChange(() => {
      if (isActiveRef.current) {
        scheduleFitDuringResize()
      }
    })

    return () => disposable.dispose()
  }, [isActiveRef, scheduleFitDuringResize, workspacePanelApi])

  useEffect(() => {
    const container = containerRef.current
    const surface = surfaceRef.current
    const resizeObserver = resizeObserverRef.current
    if (!container || !resizeObserver) return

    if (isActive) {
      resizeObserver.observe(container)
      if (surface) {
        resizeObserver.observe(surface)
      }
      activateFitTimerRef.current = window.setTimeout(() => {
        activateFitTimerRef.current = null
        scheduleFitDuringResize()
        termRef.current?.focus()
      }, TAB_ACTIVATE_REFIT_DELAY_MS)
      return
    }

    resizeObserver.unobserve(container)
    if (surface) {
      resizeObserver.unobserve(surface)
    }

    if (resizeRafRef.current !== null) {
      window.cancelAnimationFrame(resizeRafRef.current)
      resizeRafRef.current = null
    }

    if (activateFitTimerRef.current !== null) {
      window.clearTimeout(activateFitTimerRef.current)
      activateFitTimerRef.current = null
    }
  }, [
    activateFitTimerRef,
    containerRef,
    isActive,
    resizeObserverRef,
    resizeRafRef,
    scheduleFitDuringResize,
    surfaceRef,
    termRef,
  ])

  // Looking at the tab answers whatever it wanted attention for.
  useEffect(() => {
    if (!isActive) return
    hasBeenActiveRef.current = true
    clearTabAttention(tabId)
  }, [hasBeenActiveRef, isActive, tabId])

  useEffect(() => registerTerminalFocus(tabId, () => termRef.current?.focus()), [tabId, termRef])

  useEffect(() => {
    if (!isGlobalShortcutTarget || !isActive) return
    return markActiveTerminal(tabId)
  }, [isActive, isGlobalShortcutTarget, tabId])

  // Focusing a pane already in sight, from the tab bar, a shortcut or a
  // closed tab, moves no keyboard focus by itself. Focus the user put in the
  // tab, in its SFTP drawer or search bar, stays there.
  useEffect(() => {
    if (!isGlobalShortcutTarget || !isActive) return
    const focused = document.activeElement
    if (focused && shellRef.current?.contains(focused)) return
    termRef.current?.focus()
  }, [isActive, isGlobalShortcutTarget, shellRef, termRef])

  // A pane in sight next to the focused one is announced until focused.
  useEffect(() => {
    if (isGlobalShortcutTarget) dismissTabToast(tabId)
  }, [isGlobalShortcutTarget, tabId])

  useEffect(
    () => () => {
      if (bellFlashTimerRef.current !== null) window.clearTimeout(bellFlashTimerRef.current)
      clearTabAttention(tabId)
      dismissTabToast(tabId)
    },
    [bellFlashTimerRef, tabId]
  )
}
