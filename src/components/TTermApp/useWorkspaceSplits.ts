import { useCallback, useEffect } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import { useKeymap } from "@/contexts/KeymapContext"
import type { SplitDirection } from "@/lib/tabGroups"

/** Splitting a pane, from the menus or the keyboard, and dropping a tab dragged off the tab bar. */
export function useWorkspaceSplits(shell: AppShell) {
  const { workspaceRef, activeTabId, duplicateTab } = shell

  // A split opens a copy of the pane beside it, as iTerm2 and Windows Terminal do.
  const handleSplitTab = useCallback(
    (tabId: string, direction: SplitDirection) => {
      duplicateTab(tabId, direction)
    },
    [duplicateTab]
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
  }
}
