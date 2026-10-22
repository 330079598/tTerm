import { useEffect, useMemo, useState } from "react"

import { layoutPaneTree, type PaneBox, type PaneLayout } from "@/lib/paneLayout"
import { getPaneIds, type TabGroup } from "@/lib/tabGroups"

export interface PaneSlot {
  group: TabGroup
  /** Where the pane sits; a pane out of sight keeps the place its tab gives it. */
  box: PaneBox | undefined
  isVisible: boolean
  paneNumber: number
  paneCount: number
}

function useElementSize(ref: React.RefObject<HTMLElement | null>) {
  const [size, setSize] = useState({ width: 0, height: 0 })
  useEffect(() => {
    const element = ref.current
    if (!element) return
    const observer = new ResizeObserver(() => {
      const width = element.clientWidth
      const height = element.clientHeight
      setSize((current) =>
        current.width === width && current.height === height ? current : { width, height }
      )
    })
    observer.observe(element)
    return () => observer.disconnect()
  }, [ref])
  return size
}

/**
 * Where every pane of every tab sits in the workspace, which panes are in
 * sight, and where the dividers of the tab in sight are.
 */
export function usePaneSlots(
  rootRef: React.RefObject<HTMLElement | null>,
  groups: TabGroup[],
  activeTabId: string | null,
  gap: number
) {
  const size = useElementSize(rootRef)

  const layouts = useMemo(() => {
    const area = { left: 0, top: 0, width: size.width, height: size.height }
    return new Map<string, PaneLayout>(
      groups.map((group) => [group.id, layoutPaneTree(group.root, area, gap)])
    )
  }, [gap, groups, size])

  return useMemo(() => {
    const activeGroup = activeTabId
      ? groups.find((group) => getPaneIds(group.root).includes(activeTabId))
      : undefined
    const full = { left: 0, top: 0, width: size.width, height: size.height }
    const slots = new Map<string, PaneSlot>()
    const visibleBoxes = new Map<string, PaneBox>()
    for (const group of groups) {
      const paneIds = getPaneIds(group.root)
      const panes = layouts.get(group.id)?.panes
      paneIds.forEach((tabId, index) => {
        const isZoomed = group.zoomedTabId === tabId
        const slot: PaneSlot = {
          group,
          box: isZoomed ? full : panes?.get(tabId),
          isVisible: group === activeGroup && (!group.zoomedTabId || isZoomed),
          paneNumber: index + 1,
          paneCount: paneIds.length,
        }
        slots.set(tabId, slot)
        if (slot.isVisible && slot.box) visibleBoxes.set(tabId, slot.box)
      })
    }
    const sashes =
      activeGroup && !activeGroup.zoomedTabId ? (layouts.get(activeGroup.id)?.sashes ?? []) : []
    return { activeGroup, slots, visibleBoxes, sashes }
  }, [activeTabId, groups, layouts, size])
}
