import { useCallback, useState } from "react"

import {
  findDropTarget,
  type WorkspaceDropPreview,
  type WorkspaceDropTarget,
} from "@/components/TTermApp/workspaceDrop"
import type { TabPanelsProps } from "@/components/TTermApp/workspaceContext"
import type { PaneBox } from "@/lib/paneLayout"
import { findGroupOfTab, getPaneIds, type TabGroup } from "@/lib/tabGroups"
import { isSessionTab, type Tab } from "@/types/tab"

type DragSource = "tab" | "pane"

interface WorkspaceDropOptions {
  rootRef: React.RefObject<HTMLDivElement | null>
  groups: TabGroup[]
  activeGroup: TabGroup | undefined
  /** The panes in sight and where they sit in the workspace. */
  visibleBoxes: ReadonlyMap<string, PaneBox>
  tabs: Tab[]
  duplicateTab: TabPanelsProps["duplicateTab"]
  mergeGroup: TabPanelsProps["mergeGroup"]
  placePane: TabPanelsProps["placePane"]
}

/**
 * Dropping onto the tab in sight: a tab dragged off the tab bar joins it as
 * panes, and a pane dragged by its header moves within it. Dropping a lone
 * tab onto itself splits off a copy, as the split buttons do.
 */
export function useWorkspaceDrop({
  rootRef,
  groups,
  activeGroup,
  visibleBoxes,
  tabs,
  duplicateTab,
  mergeGroup,
  placePane,
}: WorkspaceDropOptions) {
  const [dropPreview, setDropPreview] = useState<WorkspaceDropPreview | null>(null)

  const clearDrop = useCallback(() => setDropPreview(null), [])

  const findTarget = useCallback(
    (source: DragSource, tabId: string, clientX: number, clientY: number) => {
      const root = rootRef.current
      if (!root || !activeGroup) return null
      const rect = root.getBoundingClientRect()
      const target = findDropTarget(visibleBoxes, clientX - rect.left, clientY - rect.top)
      if (!target) return null
      if (source === "pane") {
        return target.tabId === tabId ? null : target
      }
      if (findGroupOfTab(groups, tabId) !== activeGroup) return target
      const tab = tabs.find((candidate) => candidate.id === tabId)
      return getPaneIds(activeGroup.root).length === 1 && isSessionTab(tab) ? target : null
    },
    [activeGroup, groups, rootRef, tabs, visibleBoxes]
  )

  const previewDrop = useCallback(
    (source: DragSource, tabId: string, clientX: number, clientY: number) => {
      const target = findTarget(source, tabId, clientX, clientY)
      if (!target) {
        clearDrop()
        return false
      }
      setDropPreview((current) =>
        current &&
        current.direction === target.direction &&
        current.targetTabId === target.tabId &&
        current.sourceTabId === tabId &&
        current.left === target.box.left &&
        current.top === target.box.top &&
        current.width === target.box.width &&
        current.height === target.box.height
          ? current
          : {
              ...target.box,
              direction: target.direction,
              sourceTabId: tabId,
              targetTabId: target.tabId,
            }
      )
      return true
    },
    [clearDrop, findTarget]
  )

  const drop = useCallback(
    (source: DragSource, tabId: string, target: WorkspaceDropTarget) => {
      if (source === "pane") {
        placePane(tabId, target.tabId, target.direction)
      } else if (target.tabId === tabId) {
        duplicateTab(tabId, target.direction)
      } else {
        mergeGroup(tabId, target.tabId, target.direction)
      }
    },
    [duplicateTab, mergeGroup, placePane]
  )

  const commitDrop = useCallback(
    (source: DragSource, tabId: string, clientX: number, clientY: number) => {
      const target = findTarget(source, tabId, clientX, clientY)
      clearDrop()
      if (!target) return false
      drop(source, tabId, target)
      return true
    },
    [clearDrop, drop, findTarget]
  )

  const previewTabDrop = useCallback(
    (tabId: string, clientX: number, clientY: number) =>
      previewDrop("tab", tabId, clientX, clientY),
    [previewDrop]
  )
  const commitTabDrop = useCallback(
    (tabId: string, clientX: number, clientY: number) => commitDrop("tab", tabId, clientX, clientY),
    [commitDrop]
  )
  const previewPaneDrop = useCallback(
    (tabId: string, clientX: number, clientY: number) =>
      previewDrop("pane", tabId, clientX, clientY),
    [previewDrop]
  )
  const commitPaneDrop = useCallback(
    (tabId: string, clientX: number, clientY: number) =>
      commitDrop("pane", tabId, clientX, clientY),
    [commitDrop]
  )

  return {
    dropPreview,
    clearDrop,
    previewTabDrop,
    commitTabDrop,
    previewPaneDrop,
    commitPaneDrop,
  }
}
