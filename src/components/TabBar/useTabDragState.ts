import { useEffect, useRef, useState } from "react"

import type { Tab } from "@/types/tab"

export type TabDragMode = "list" | "detached"

export type TabDragState = {
  tabId: string
  pointerId: number
  startX: number
  startY: number
  dragging: boolean
  mode: TabDragMode
  sourceElement: HTMLDivElement
  threshold: number
  // Where the pointer grabbed the tab, measured from the tab's left edge.
  grabOffsetX: number
  gap: number
  // The index the tab lands on if dropped now; null while detached.
  toIndex: number | null
}

/** A tab drag's state: where it is, and the refs its pointer handlers share. */
export function useTabDragState(tabs: Tab[]) {
  const dragStateRef = useRef<TabDragState | null>(null)
  const dragGhostRef = useRef<HTMLDivElement | null>(null)
  const lastPointerPosRef = useRef<{ x: number; y: number } | null>(null)
  const autoScrollRef = useRef<{ frame: number | null; speed: number }>({ frame: null, speed: 0 })
  // Each tab's on-screen left edge as the drag ended, so the next layout can
  // slide every tab from there into its slot instead of jumping (FLIP).
  const pendingFlipRef = useRef<{ lefts: Map<string, number>; raisedTabId: string } | null>(null)
  const tabsRef = useRef(tabs)
  const suppressNextClickRef = useRef(false)
  const [draggingTabId, setDraggingTabId] = useState<string | null>(null)
  const [dragMode, setDragMode] = useState<TabDragMode | null>(null)
  const [dragGhostTab, setDragGhostTab] = useState<Tab | null>(null)

  useEffect(() => {
    tabsRef.current = tabs
  }, [tabs])

  useEffect(() => {
    if (dragGhostTab && dragGhostRef.current && lastPointerPosRef.current) {
      const { x, y } = lastPointerPosRef.current
      dragGhostRef.current.style.transform = `translate(${x}px, ${y}px)`
    }
  }, [dragGhostTab])

  return {
    dragStateRef,
    dragGhostRef,
    lastPointerPosRef,
    autoScrollRef,
    pendingFlipRef,
    tabsRef,
    suppressNextClickRef,
    draggingTabId,
    setDraggingTabId,
    dragMode,
    setDragMode,
    dragGhostTab,
    setDragGhostTab,
  }
}
