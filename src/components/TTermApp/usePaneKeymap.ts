import { useEffect, useRef } from "react"
import type { DockviewApi } from "dockview-react"

import { useKeymap } from "@/contexts/KeymapContext"
import type { KeymapActionId } from "@/lib/keymap/actions"
import {
  findPaneInDirection,
  getSiblingPaneId,
  type PaneDirection,
  type PaneRect,
} from "@/lib/paneNavigation"
import { nudgeTab } from "@/lib/tabNudge"

const DIRECTION_ACTIONS: [KeymapActionId, PaneDirection][] = [
  ["workspace.focusPaneLeft", "left"],
  ["workspace.focusPaneRight", "right"],
  ["workspace.focusPaneUp", "up"],
  ["workspace.focusPaneDown", "down"],
]

function paneRects(dockApi: DockviewApi): PaneRect[] {
  return dockApi.groups.map((group) => {
    const { left, top, right, bottom } = group.element.getBoundingClientRect()
    return { id: group.id, left, top, right, bottom }
  })
}

/**
 * Keyboard shortcuts that move the focus between split panes, in reading
 * order or by direction, and that gather every pane back into one. Activating
 * a pane's tab puts the keyboard in its terminal, as clicking the pane does.
 */
export function usePaneKeymap(
  dockApi: DockviewApi | null,
  collapseToSingleGroup: (tabId: string) => void
) {
  const { registerHandler } = useKeymap()
  // Pane ids from most to least recently focused, to break ties between
  // neighbours the way tmux does.
  const recentPaneIdsRef = useRef<string[]>([])

  useEffect(() => {
    if (!dockApi) return
    const remember = (groupId: string) => {
      recentPaneIdsRef.current = [
        groupId,
        ...recentPaneIdsRef.current.filter((id) => id !== groupId),
      ]
    }
    if (dockApi.activeGroup) remember(dockApi.activeGroup.id)
    const disposable = dockApi.onDidActiveGroupChange((group) => {
      if (group) remember(group.id)
    })
    return () => disposable.dispose()
  }, [dockApi])

  useEffect(() => {
    if (!dockApi) return

    // A shortcut with no pane to go to, or no split to undo, flashes the
    // active tab, so it still shows it was received.
    const focusPane = (pickPane: (panes: PaneRect[], activeId: string) => string | undefined) => {
      const activeGroup = dockApi.activeGroup
      if (!activeGroup) return false
      const targetId = pickPane(paneRects(dockApi), activeGroup.id)
      const target = targetId ? dockApi.groups.find((group) => group.id === targetId) : undefined
      if (target?.activePanel) {
        target.activePanel.api.setActive()
      } else if (activeGroup.activePanel) {
        nudgeTab(activeGroup.activePanel.id)
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
      registerHandler("workspace.restoreSingleGroup", () => {
        const activePanel = dockApi.activePanel
        if (!activePanel) return false
        if (dockApi.groups.length > 1) {
          collapseToSingleGroup(activePanel.id)
        } else {
          nudgeTab(activePanel.id)
        }
      }),
    ]
    return () => unregisters.forEach((unregister) => unregister())
  }, [collapseToSingleGroup, dockApi, registerHandler])
}
