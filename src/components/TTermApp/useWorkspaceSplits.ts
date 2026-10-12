import { useCallback, useEffect } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { WorkspaceSplitDirection } from "@/components/TTermApp/workspaceContext"
import { useKeymap } from "@/contexts/KeymapContext"

/** Splitting the workspace, from the menus, the keyboard or a dragged tab. */
export function useWorkspaceSplits(shell: AppShell) {
  const { workspaceRef, tabs, activeTabId } = shell

  const handleSplitTab = useCallback(
    (tabId: string, direction: WorkspaceSplitDirection) => {
      workspaceRef.current?.splitTab(tabId, direction)
    },
    [workspaceRef]
  )

  const handleTabDragMove = useCallback(
    (tabId: string, clientX: number, clientY: number) => {
      workspaceRef.current?.previewTabDrop(tabId, clientX, clientY)
    },
    [workspaceRef]
  )

  const handleTabDrop = useCallback(
    (tabId: string, clientX: number, clientY: number) => {
      return workspaceRef.current?.commitTabDrop(tabId, clientX, clientY) ?? false
    },
    [workspaceRef]
  )

  const handleTabDragCancel = useCallback(() => {
    workspaceRef.current?.clearTabDrop()
  }, [workspaceRef])

  const getTabContextIds = useCallback(
    (tabId: string) => workspaceRef.current?.getGroupTabIds(tabId) ?? tabs.map((tab) => tab.id),
    [tabs, workspaceRef]
  )

  const { registerHandler } = useKeymap()

  useEffect(() => {
    const unregisterSplitRight = registerHandler("workspace.splitRight", () => {
      if (activeTabId) {
        handleSplitTab(activeTabId, "right")
      }
    })
    const unregisterSplitBelow = registerHandler("workspace.splitBelow", () => {
      if (activeTabId) {
        handleSplitTab(activeTabId, "below")
      }
    })
    return () => {
      unregisterSplitRight()
      unregisterSplitBelow()
    }
  }, [registerHandler, handleSplitTab, activeTabId])

  return {
    handleSplitTab,
    handleTabDragMove,
    handleTabDrop,
    handleTabDragCancel,
    getTabContextIds,
  }
}
