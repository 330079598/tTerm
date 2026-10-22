import { useEffect, useRef } from "react"

import { useKeymap } from "@/contexts/KeymapContext"
import type { KeymapActionId } from "@/lib/keymap/actions"
import {
  findPaneInDirection,
  getSiblingPaneId,
  type PaneDirection,
  type PaneRect,
} from "@/lib/paneNavigation"
import type { PaneBox } from "@/lib/paneLayout"
import { getPaneIds, type TabGroup } from "@/lib/tabGroups"
import { nudgeTab } from "@/lib/tabNudge"

const DIRECTION_ACTIONS: [KeymapActionId, PaneDirection][] = [
  ["workspace.focusPaneLeft", "left"],
  ["workspace.focusPaneRight", "right"],
  ["workspace.focusPaneUp", "up"],
  ["workspace.focusPaneDown", "down"],
]

interface PaneKeymapOptions {
  activeTabId: string | null
  activeGroup: TabGroup | undefined
  /** The panes in sight and where they sit. */
  visibleBoxes: ReadonlyMap<string, PaneBox>
  setActiveTab: (tabId: string) => void
  ungroupPanes: (tabId: string) => void
  togglePaneZoom: (tabId: string) => void
  breakOutPane: (tabId: string) => void
}

function paneRects(boxes: ReadonlyMap<string, PaneBox>): PaneRect[] {
  return [...boxes].map(([id, box]) => ({
    id,
    left: box.left,
    top: box.top,
    right: box.left + box.width,
    bottom: box.top + box.height,
  }))
}

/**
 * Keyboard shortcuts for the panes of the tab in sight: moving the focus
 * between them in reading order or by direction, zooming one, moving one out
 * to a tab of its own, and undoing the split. Focusing a pane puts the
 * keyboard in its terminal, as clicking it does.
 */
export function usePaneKeymap({
  activeTabId,
  activeGroup,
  visibleBoxes,
  setActiveTab,
  ungroupPanes,
  togglePaneZoom,
  breakOutPane,
}: PaneKeymapOptions) {
  const { registerHandler } = useKeymap()
  // Pane ids from most to least recently focused, to break ties between
  // neighbours the way tmux does.
  const recentPaneIdsRef = useRef<string[]>([])

  useEffect(() => {
    if (!activeTabId) return
    recentPaneIdsRef.current = [
      activeTabId,
      ...recentPaneIdsRef.current.filter((id) => id !== activeTabId),
    ]
  }, [activeTabId])

  useEffect(() => {
    if (!activeTabId) return
    const isSplit = activeGroup !== undefined && getPaneIds(activeGroup.root).length > 1

    // A shortcut with no pane to go to, or no split to act on, flashes the
    // active tab, so it still shows it was received.
    const focusPane = (pickPane: (panes: PaneRect[], activeId: string) => string | undefined) => {
      const targetId = pickPane(paneRects(visibleBoxes), activeTabId)
      if (targetId && targetId !== activeTabId) {
        setActiveTab(targetId)
      } else {
        nudgeTab(activeTabId)
      }
    }
    const onSplit = (act: (tabId: string) => void) => () => {
      if (isSplit) {
        act(activeTabId)
      } else {
        nudgeTab(activeTabId)
      }
    }

    const unregisters = [
      registerHandler("workspace.focusNextPane", () =>
        focusPane((panes, activeId) => getSiblingPaneId(panes, activeId, 1))
      ),
      registerHandler("workspace.focusPrevPane", () =>
        focusPane((panes, activeId) => getSiblingPaneId(panes, activeId, -1))
      ),
      ...DIRECTION_ACTIONS.map(([actionId, direction]) =>
        registerHandler(actionId, () =>
          focusPane((panes, activeId) =>
            findPaneInDirection(panes, activeId, direction, recentPaneIdsRef.current)
          )
        )
      ),
      registerHandler("workspace.restoreSingleGroup", onSplit(ungroupPanes)),
      registerHandler("workspace.togglePaneZoom", onSplit(togglePaneZoom)),
      registerHandler("workspace.movePaneToNewTab", onSplit(breakOutPane)),
    ]
    return () => unregisters.forEach((unregister) => unregister())
  }, [
    activeGroup,
    activeTabId,
    breakOutPane,
    registerHandler,
    setActiveTab,
    togglePaneZoom,
    ungroupPanes,
    visibleBoxes,
  ])
}
