import React, { useCallback, useEffect, useLayoutEffect } from "react"

import type {
  TabDragMode,
  TabDragState,
  useTabDragState,
} from "@/components/TabBar/useTabDragState"
import type { TabBarProps } from "@/components/TabBar"

const TAB_DRAG_THRESHOLD = 6
// How far the pointer may stray above or below the tab list before the tab
// detaches from it and becomes a ghost that can be dropped on the workspace.
const TAB_DRAG_DETACH_DISTANCE = 24
const TAB_DRAG_AUTO_SCROLL_EDGE = 40
const TAB_DRAG_AUTO_SCROLL_MAX_SPEED = 14

// A tab's offsetLeft counts from the list's content box, so pointer positions
// are put on the same footing, independent of how far the list has scrolled.
function getListContentX(list: HTMLElement, clientX: number): number {
  return clientX - list.getBoundingClientRect().left - list.clientLeft + list.scrollLeft
}

type TabDragOptions = Pick<
  ReturnType<typeof useTabDragState>,
  | "dragStateRef"
  | "dragGhostRef"
  | "lastPointerPosRef"
  | "autoScrollRef"
  | "pendingFlipRef"
  | "tabsRef"
  | "suppressNextClickRef"
  | "setDraggingTabId"
  | "setDragMode"
  | "setDragGhostTab"
> &
  Pick<TabBarProps, "onTabDragMove" | "onTabDrop" | "onTabDragCancel" | "onTabMove"> & {
    listRef: React.RefObject<HTMLDivElement | null>
    viewportRef: React.RefObject<HTMLDivElement | null>
  }

/**
 * Dragging a tab: along the list to reorder it, scrolling the list at its
 * edges, or off the list onto the workspace; the tabs slide into their new
 * places once it is dropped.
 */
export function useTabDrag({
  dragStateRef,
  dragGhostRef,
  lastPointerPosRef,
  autoScrollRef,
  pendingFlipRef,
  tabsRef,
  suppressNextClickRef,
  setDraggingTabId,
  setDragMode,
  setDragGhostTab,
  onTabDragMove,
  onTabDrop,
  onTabDragCancel,
  onTabMove,
  listRef,
  viewportRef,
}: TabDragOptions) {
  const getTabElements = useCallback(
    () => Array.from(listRef.current?.querySelectorAll<HTMLElement>("[data-tab-id]") ?? []),
    [listRef]
  )

  const captureTabPositions = useCallback(
    (raisedTabId: string) => {
      pendingFlipRef.current = {
        lefts: new Map(
          getTabElements().map((element) => [
            element.dataset.tabId ?? "",
            element.getBoundingClientRect().left,
          ])
        ),
        raisedTabId,
      }
    },
    [getTabElements, pendingFlipRef]
  )

  useLayoutEffect(() => {
    const pendingFlip = pendingFlipRef.current
    if (!pendingFlip) {
      return
    }
    pendingFlipRef.current = null
    const previousLefts = pendingFlip.lefts

    const elements = getTabElements()
    for (const element of elements) {
      element.style.transition = "none"
      element.style.transform = ""
    }
    const offsets = elements.map((element) => {
      const previousLeft = previousLefts.get(element.dataset.tabId ?? "")
      return previousLeft === undefined ? 0 : previousLeft - element.getBoundingClientRect().left
    })
    elements.forEach((element, index) => {
      if (Math.abs(offsets[index]) >= 0.5) {
        element.style.transform = `translateX(${offsets[index]}px)`
      }
    })
    // Commit the inverted positions before letting them transition back to 0.
    void listRef.current?.offsetWidth
    for (const element of elements) {
      element.style.transition = ""
      element.style.transform = ""
    }

    // The dropped tab has lost its dragging z-index by now; keep it above its
    // neighbours (the active tab sits raised too) until it has settled.
    const droppedTab = elements.find((element) => element.dataset.tabId === pendingFlip.raisedTabId)
    if (droppedTab && typeof droppedTab.getAnimations === "function") {
      const settling = droppedTab
        .getAnimations()
        .find(
          (animation) =>
            animation instanceof CSSTransition && animation.transitionProperty === "transform"
        )
      if (settling) {
        droppedTab.style.zIndex = "3"
        const lower = () => {
          droppedTab.style.zIndex = ""
        }
        settling.finished.then(lower, lower)
      }
    }
  })

  const stopAutoScroll = useCallback(() => {
    const autoScroll = autoScrollRef.current
    if (autoScroll.frame !== null) {
      cancelAnimationFrame(autoScroll.frame)
    }
    autoScroll.frame = null
    autoScroll.speed = 0
  }, [autoScrollRef])

  // Follows the pointer with the dragged tab inside the list, and slides the
  // tabs it passes over out of the way by one tab width plus the gap.
  const layoutListDrag = useCallback(
    (dragState: TabDragState, clientX: number) => {
      const list = listRef.current
      const elements = getTabElements()
      const source = dragState.sourceElement
      const fromIndex = elements.indexOf(source)
      if (!list || fromIndex < 0) {
        return
      }

      const first = elements[0]
      const last = elements[elements.length - 1]
      const offset = Math.min(
        Math.max(
          getListContentX(list, clientX) - dragState.grabOffsetX - source.offsetLeft,
          first.offsetLeft - source.offsetLeft
        ),
        last.offsetLeft + last.offsetWidth - source.offsetLeft - source.offsetWidth
      )
      source.style.transform = `translateX(${offset}px)`

      // A tab is passed once the dragged tab's leading edge crosses its middle.
      // Comparing centers instead would strand a wide tab that cannot travel
      // past the narrower first or last tab, since the drag stops at the ends.
      const draggedLeft = source.offsetLeft + offset
      const draggedRight = draggedLeft + source.offsetWidth
      let toIndex = 0
      elements.forEach((element, index) => {
        const elementCenter = element.offsetLeft + element.offsetWidth / 2
        if (
          (index < fromIndex && elementCenter <= draggedLeft) ||
          (index > fromIndex && elementCenter < draggedRight)
        ) {
          toIndex += 1
        }
      })

      const shift = source.offsetWidth + dragState.gap
      elements.forEach((element, index) => {
        if (index === fromIndex) {
          return
        }
        const elementShift =
          index > fromIndex && index <= toIndex
            ? -shift
            : index < fromIndex && index >= toIndex
              ? shift
              : 0
        element.style.transform = elementShift ? `translateX(${elementShift}px)` : ""
      })
      dragState.toIndex = toIndex
    },
    [getTabElements, listRef]
  )

  // Closes the gap the detached tab left behind.
  const layoutDetachedDrag = useCallback(
    (dragState: TabDragState) => {
      const elements = getTabElements()
      const source = dragState.sourceElement
      const fromIndex = elements.indexOf(source)
      const shift = source.offsetWidth + dragState.gap
      elements.forEach((element, index) => {
        element.style.transform =
          fromIndex >= 0 && index > fromIndex ? `translateX(${-shift}px)` : ""
      })
      dragState.toIndex = null
    },
    [getTabElements]
  )

  const updateAutoScroll = useCallback(
    (clientX: number) => {
      const list = listRef.current
      const viewport = viewportRef.current
      const autoScroll = autoScrollRef.current
      if (!list || !viewport || list.scrollWidth <= list.clientWidth) {
        stopAutoScroll()
        return
      }

      const rect = viewport.getBoundingClientRect()
      const edge = TAB_DRAG_AUTO_SCROLL_EDGE
      const leftDepth = rect.left + edge - clientX
      const rightDepth = clientX - (rect.right - edge)
      autoScroll.speed =
        leftDepth > 0
          ? -Math.min(1, leftDepth / edge) * TAB_DRAG_AUTO_SCROLL_MAX_SPEED
          : rightDepth > 0
            ? Math.min(1, rightDepth / edge) * TAB_DRAG_AUTO_SCROLL_MAX_SPEED
            : 0

      if (autoScroll.speed === 0) {
        stopAutoScroll()
        return
      }
      if (autoScroll.frame !== null) {
        return
      }

      const step = () => {
        const dragState = dragStateRef.current
        const pointer = lastPointerPosRef.current
        if (!dragState || dragState.mode !== "list" || !pointer || autoScroll.speed === 0) {
          autoScroll.frame = null
          return
        }
        const scrollLeft = list.scrollLeft
        list.scrollLeft += autoScroll.speed
        if (list.scrollLeft !== scrollLeft) {
          layoutListDrag(dragState, pointer.x)
        }
        autoScroll.frame = requestAnimationFrame(step)
      }
      autoScroll.frame = requestAnimationFrame(step)
    },
    [
      layoutListDrag,
      stopAutoScroll,
      autoScrollRef,
      dragStateRef,
      lastPointerPosRef,
      listRef,
      viewportRef,
    ]
  )

  const resetTabDrag = useCallback(() => {
    dragStateRef.current = null
    stopAutoScroll()
    setDraggingTabId(null)
    setDragMode(null)
    setDragGhostTab(null)
    onTabDragCancel()
  }, [
    onTabDragCancel,
    stopAutoScroll,
    dragStateRef,
    setDragGhostTab,
    setDragMode,
    setDraggingTabId,
  ])

  const cancelTabDrag = useCallback(() => {
    const dragState = dragStateRef.current
    if (!dragState) {
      return
    }

    if (dragState.sourceElement.hasPointerCapture(dragState.pointerId)) {
      dragState.sourceElement.releasePointerCapture(dragState.pointerId)
    }
    if (dragState.dragging) {
      captureTabPositions(dragState.tabId)
    }
    resetTabDrag()
  }, [captureTabPositions, resetTabDrag, dragStateRef])

  const handleTabPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>, tabId: string) => {
      const target = event.target
      if (event.button !== 0 || !(target instanceof Element) || target.closest("button")) {
        return
      }

      suppressNextClickRef.current = false
      dragStateRef.current = {
        tabId,
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        dragging: false,
        mode: "list",
        sourceElement: event.currentTarget,
        threshold: TAB_DRAG_THRESHOLD * (window.devicePixelRatio || 1),
        // Measured on screen rather than from offsetLeft, so a tab grabbed
        // while still sliding into place stays under the pointer.
        grabOffsetX: event.clientX - event.currentTarget.getBoundingClientRect().left,
        gap: 0,
        toIndex: null,
      }
      event.currentTarget.setPointerCapture(event.pointerId)
    },
    [dragStateRef, suppressNextClickRef]
  )

  const updateTabDrag = useCallback(
    (dragState: TabDragState, clientX: number, clientY: number) => {
      const listRect = listRef.current?.getBoundingClientRect()
      const mode: TabDragMode =
        listRect &&
        clientY >= listRect.top - TAB_DRAG_DETACH_DISTANCE &&
        clientY <= listRect.bottom + TAB_DRAG_DETACH_DISTANCE
          ? "list"
          : "detached"

      if (mode !== dragState.mode) {
        dragState.mode = mode
        setDragMode(mode)
        if (mode === "detached") {
          setDragGhostTab(tabsRef.current.find((tab) => tab.id === dragState.tabId) ?? null)
        } else {
          setDragGhostTab(null)
          onTabDragCancel()
        }
      }

      if (mode === "list") {
        layoutListDrag(dragState, clientX)
        updateAutoScroll(clientX)
        return
      }

      stopAutoScroll()
      layoutDetachedDrag(dragState)
      const ghost = dragGhostRef.current
      if (ghost) {
        ghost.style.transform = `translate(${clientX}px, ${clientY}px)`
      }
      onTabDragMove(dragState.tabId, clientX, clientY)
    },
    [
      layoutDetachedDrag,
      layoutListDrag,
      onTabDragCancel,
      onTabDragMove,
      stopAutoScroll,
      updateAutoScroll,
      dragGhostRef,
      listRef,
      setDragGhostTab,
      setDragMode,
      tabsRef,
    ]
  )

  const handleTabPointerMove = useCallback(
    (event: PointerEvent) => {
      const dragState = dragStateRef.current
      if (!dragState || dragState.pointerId !== event.pointerId) {
        return
      }

      const deltaX = event.clientX - dragState.startX
      const deltaY = event.clientY - dragState.startY
      lastPointerPosRef.current = { x: event.clientX, y: event.clientY }

      if (!dragState.dragging) {
        if (Math.hypot(deltaX, deltaY) < dragState.threshold) {
          return
        }
        dragState.dragging = true
        dragState.gap = listRef.current
          ? parseFloat(getComputedStyle(listRef.current).columnGap) || 0
          : 0
        suppressNextClickRef.current = true
        setDraggingTabId(dragState.tabId)
        setDragMode("list")
      }

      event.preventDefault()
      updateTabDrag(dragState, event.clientX, event.clientY)
    },
    [
      updateTabDrag,
      dragStateRef,
      lastPointerPosRef,
      listRef,
      setDragMode,
      setDraggingTabId,
      suppressNextClickRef,
    ]
  )

  const finishTabDrag = useCallback(
    (event: PointerEvent) => {
      const dragState = dragStateRef.current
      if (!dragState || dragState.pointerId !== event.pointerId) {
        return
      }

      if (dragState.sourceElement.hasPointerCapture(event.pointerId)) {
        dragState.sourceElement.releasePointerCapture(event.pointerId)
      }

      if (dragState.dragging) {
        event.preventDefault()
        if (dragState.mode === "detached") {
          onTabDrop(dragState.tabId, event.clientX, event.clientY)
        }

        captureTabPositions(dragState.tabId)
        const fromIndex = tabsRef.current.findIndex((tab) => tab.id === dragState.tabId)
        const toIndex = dragState.toIndex
        if (fromIndex >= 0 && toIndex !== null && fromIndex !== toIndex) {
          onTabMove(fromIndex, toIndex)
        }
      }

      resetTabDrag()
    },
    [captureTabPositions, onTabDrop, onTabMove, resetTabDrag, dragStateRef, tabsRef]
  )

  useEffect(() => {
    const handlePointerCancel = (event: PointerEvent) => {
      if (dragStateRef.current?.pointerId === event.pointerId) {
        cancelTabDrag()
      }
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && dragStateRef.current?.dragging) {
        event.preventDefault()
        event.stopPropagation()
        cancelTabDrag()
      }
    }

    window.addEventListener("pointermove", handleTabPointerMove, true)
    window.addEventListener("pointerup", finishTabDrag, true)
    window.addEventListener("pointercancel", handlePointerCancel, true)
    window.addEventListener("keydown", handleKeyDown, true)
    window.addEventListener("blur", cancelTabDrag)

    return () => {
      window.removeEventListener("pointermove", handleTabPointerMove, true)
      window.removeEventListener("pointerup", finishTabDrag, true)
      window.removeEventListener("pointercancel", handlePointerCancel, true)
      window.removeEventListener("keydown", handleKeyDown, true)
      window.removeEventListener("blur", cancelTabDrag)
    }
  }, [cancelTabDrag, finishTabDrag, handleTabPointerMove, dragStateRef])

  useEffect(() => stopAutoScroll, [stopAutoScroll])

  return { layoutListDrag, handleTabPointerDown }
}
