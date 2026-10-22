import { useCallback, useEffect, useRef, useState } from "react"

import { getTabBarDropIndex } from "@/components/TTermApp/workspaceDrop"
import { useLatestRef } from "@/hooks/useLatestRef"
import type { Tab } from "@/types/tab"

const PANE_DRAG_THRESHOLD = 6

interface PaneDragOptions {
  tabs: Tab[]
  previewPaneDrop: (tabId: string, clientX: number, clientY: number) => boolean
  commitPaneDrop: (tabId: string, clientX: number, clientY: number) => boolean
  clearDrop: () => void
  breakOutPane: (tabId: string, index?: number) => void
}

interface PaneDragState {
  tabId: string
  pointerId: number
  startX: number
  startY: number
  dragging: boolean
  element: HTMLElement
}

function isOverTitleBar(clientY: number) {
  const titleBar = document.querySelector(".title-bar")
  return titleBar ? clientY <= titleBar.getBoundingClientRect().bottom : false
}

function tabBarCenters() {
  return Array.from(document.querySelectorAll<HTMLElement>(".tab-list [data-tab-id]")).map(
    (element) => {
      const rect = element.getBoundingClientRect()
      return rect.left + rect.width / 2
    }
  )
}

/**
 * Dragging a pane by its header: onto a side of another pane to move it
 * there, or up onto the tab bar to give it a tab of its own. Escape cancels.
 */
export function usePaneDrag(options: PaneDragOptions) {
  const optionsRef = useRef(options)
  useLatestRef(optionsRef, options)
  const dragRef = useRef<PaneDragState | null>(null)
  const ghostRef = useRef<HTMLDivElement | null>(null)
  const [ghostTab, setGhostTab] = useState<Tab | null>(null)

  const finishDrag = useCallback(() => {
    const drag = dragRef.current
    dragRef.current = null
    if (drag?.element.hasPointerCapture(drag.pointerId)) {
      drag.element.releasePointerCapture(drag.pointerId)
    }
    document.body.classList.remove("workspace-pane-dragging", "pane-drag-over-tab-bar")
    optionsRef.current.clearDrop()
    setGhostTab(null)
  }, [])

  const startPaneDrag = useCallback((event: React.PointerEvent<HTMLElement>, tabId: string) => {
    const target = event.target
    if (event.button !== 0 || !(target instanceof Element) || target.closest("button")) return
    dragRef.current = {
      tabId,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      dragging: false,
      element: event.currentTarget,
    }
    event.currentTarget.setPointerCapture(event.pointerId)
  }, [])

  useEffect(() => {
    const handleMove = (event: PointerEvent) => {
      const drag = dragRef.current
      if (!drag || drag.pointerId !== event.pointerId) return
      if (!drag.dragging) {
        const distance = Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY)
        if (distance < PANE_DRAG_THRESHOLD * (window.devicePixelRatio || 1)) return
        drag.dragging = true
        window.getSelection()?.removeAllRanges()
        document.body.classList.add("workspace-pane-dragging")
        setGhostTab(optionsRef.current.tabs.find((tab) => tab.id === drag.tabId) ?? null)
      }
      event.preventDefault()
      if (ghostRef.current) {
        ghostRef.current.style.transform = `translate(${event.clientX}px, ${event.clientY}px)`
      }
      const overTabBar = isOverTitleBar(event.clientY)
      document.body.classList.toggle("pane-drag-over-tab-bar", overTabBar)
      if (overTabBar) {
        optionsRef.current.clearDrop()
      } else {
        optionsRef.current.previewPaneDrop(drag.tabId, event.clientX, event.clientY)
      }
    }

    const handleUp = (event: PointerEvent) => {
      const drag = dragRef.current
      if (!drag || drag.pointerId !== event.pointerId) return
      if (drag.dragging) {
        event.preventDefault()
        if (isOverTitleBar(event.clientY)) {
          optionsRef.current.breakOutPane(
            drag.tabId,
            getTabBarDropIndex(tabBarCenters(), event.clientX)
          )
        } else {
          optionsRef.current.commitPaneDrop(drag.tabId, event.clientX, event.clientY)
        }
      }
      finishDrag()
    }

    const handleCancel = (event: PointerEvent) => {
      if (dragRef.current?.pointerId === event.pointerId) finishDrag()
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && dragRef.current?.dragging) {
        event.preventDefault()
        event.stopPropagation()
        finishDrag()
      }
    }

    window.addEventListener("pointermove", handleMove, true)
    window.addEventListener("pointerup", handleUp, true)
    window.addEventListener("pointercancel", handleCancel, true)
    window.addEventListener("keydown", handleKeyDown, true)
    window.addEventListener("blur", finishDrag)
    return () => {
      window.removeEventListener("pointermove", handleMove, true)
      window.removeEventListener("pointerup", handleUp, true)
      window.removeEventListener("pointercancel", handleCancel, true)
      window.removeEventListener("keydown", handleKeyDown, true)
      window.removeEventListener("blur", finishDrag)
    }
  }, [finishDrag])

  return { startPaneDrag, ghostTab, ghostRef }
}
