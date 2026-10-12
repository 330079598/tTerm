import { useCallback } from "react"
import type { DockviewApi } from "dockview-react"

import type {
  PendingSplit,
  WorkspaceDropPreview,
  WorkspaceDropDirection,
} from "@/components/TTermApp/workspaceDrop"
import type { TabPanelsProps } from "@/components/TTermApp/workspaceContext"
import type { Tab } from "@/types/tab"
import type { WorkspaceSplitDirection } from "@/components/TTermApp/workspaceContext"

interface WorkspaceDropOptions {
  dockApi: DockviewApi | null
  tabsRef: React.RefObject<Tab[]>
  duplicateTab: TabPanelsProps["duplicateTab"]
  pendingSplitRef: React.RefObject<PendingSplit | null>
  dropPreviewRef: React.RefObject<WorkspaceDropPreview | null>
  setDropPreview: React.Dispatch<React.SetStateAction<WorkspaceDropPreview | null>>
  dockRootRef: React.RefObject<HTMLDivElement | null>
}

/**
 * Splitting a tab into a new group, and dropping a tab dragged off the tab
 * bar onto the workspace: next to a group, or into it.
 */
export function useWorkspaceDrop({
  dockApi,
  tabsRef,
  duplicateTab,
  pendingSplitRef,
  dropPreviewRef,
  setDropPreview,
  dockRootRef,
}: WorkspaceDropOptions) {
  const splitTab = useCallback(
    (tabId: string, direction: WorkspaceSplitDirection) => {
      const sourcePanel = dockApi?.getPanel(tabId)
      if (!dockApi || !sourcePanel) {
        return
      }
      const sourceTab = tabsRef.current.find((tab) => tab.id === tabId)
      if (!sourceTab || (sourceTab.type !== "terminal" && sourceTab.type !== "ssh")) {
        return
      }

      if (sourcePanel.group.panels.length === 1) {
        const duplicatedTabId = duplicateTab(tabId)
        if (duplicatedTabId) {
          pendingSplitRef.current = {
            direction,
            sourceTabId: tabId,
            tabId: duplicatedTabId,
          }
        }
        return
      }

      const targetGroup = dockApi.addGroup({ referencePanel: sourcePanel, direction })
      sourcePanel.api.moveTo({ group: targetGroup, position: "center" })
      sourcePanel.api.setActive()
    },
    [dockApi, duplicateTab, pendingSplitRef, tabsRef]
  )

  const clearTabDrop = useCallback(() => {
    dropPreviewRef.current = null
    setDropPreview(null)
  }, [dropPreviewRef, setDropPreview])

  const previewTabDrop = useCallback(
    (tabId: string, clientX: number, clientY: number) => {
      const root = dockRootRef.current
      const api = dockApi
      const sourcePanel = api?.getPanel(tabId)
      if (!root || !api || !sourcePanel) {
        clearTabDrop()
        return false
      }

      const rootRect = root.getBoundingClientRect()
      if (
        clientX < rootRect.left ||
        clientX > rootRect.right ||
        clientY < rootRect.top ||
        clientY > rootRect.bottom
      ) {
        clearTabDrop()
        return false
      }

      const targetGroupMatch = api.groups
        .map((group) => ({ group, bounds: group.api.boundingBox }))
        .filter(
          (
            candidate
          ): candidate is {
            group: (typeof api.groups)[number]
            bounds: NonNullable<(typeof candidate)["bounds"]>
          } => {
            const { bounds } = candidate
            return (
              bounds !== undefined &&
              bounds.width > 0 &&
              bounds.height > 0 &&
              clientX >= rootRect.left + bounds.left &&
              clientX <= rootRect.left + bounds.left + bounds.width &&
              clientY >= rootRect.top + bounds.top &&
              clientY <= rootRect.top + bounds.top + bounds.height
            )
          }
        )
        .sort(
          (first, second) =>
            first.bounds.width * first.bounds.height - second.bounds.width * second.bounds.height
        )[0]
      const targetGroup = targetGroupMatch?.group
      const targetPanel = targetGroup?.activePanel ?? targetGroup?.panels[0]
      if (!targetGroupMatch || !targetPanel) {
        clearTabDrop()
        return false
      }

      const targetRect = {
        left: rootRect.left + targetGroupMatch.bounds.left,
        top: rootRect.top + targetGroupMatch.bounds.top,
        width: targetGroupMatch.bounds.width,
        height: targetGroupMatch.bounds.height,
      }
      const normalizedX = (clientX - targetRect.left) / targetRect.width - 0.5
      const normalizedY = (clientY - targetRect.top) / targetRect.height - 0.5
      const isSourceGroup = sourcePanel.group === targetPanel.group
      const direction: WorkspaceDropDirection =
        !isSourceGroup && Math.abs(normalizedX) <= 0.25 && Math.abs(normalizedY) <= 0.25
          ? "center"
          : Math.abs(normalizedX) > Math.abs(normalizedY)
            ? normalizedX < 0
              ? "left"
              : "right"
            : normalizedY < 0
              ? "above"
              : "below"

      if (isSourceGroup) {
        const sourceTab = tabsRef.current.find((tab) => tab.id === tabId)
        if (
          sourcePanel.group.panels.length === 1 &&
          (!sourceTab || (sourceTab.type !== "terminal" && sourceTab.type !== "ssh"))
        ) {
          clearTabDrop()
          return false
        }
      }
      const nextPreview: WorkspaceDropPreview = {
        direction,
        sourceTabId: tabId,
        targetTabId: targetPanel.id,
        left: targetRect.left - rootRect.left,
        top: targetRect.top - rootRect.top,
        width: targetRect.width,
        height: targetRect.height,
      }

      dropPreviewRef.current = nextPreview
      setDropPreview((current) =>
        current?.direction === nextPreview.direction &&
        current.targetTabId === nextPreview.targetTabId &&
        current.sourceTabId === nextPreview.sourceTabId &&
        current.left === nextPreview.left &&
        current.top === nextPreview.top &&
        current.width === nextPreview.width &&
        current.height === nextPreview.height
          ? current
          : nextPreview
      )
      return true
    },
    [clearTabDrop, dockApi, dockRootRef, dropPreviewRef, setDropPreview, tabsRef]
  )

  const commitTabDrop = useCallback(
    (tabId: string, clientX: number, clientY: number) => {
      if (!previewTabDrop(tabId, clientX, clientY)) {
        return false
      }

      const preview = dropPreviewRef.current
      const sourcePanel = dockApi?.getPanel(tabId)
      const targetPanel = preview ? dockApi?.getPanel(preview.targetTabId) : undefined
      if (!preview || !sourcePanel || !targetPanel) {
        clearTabDrop()
        return false
      }

      if (sourcePanel.group === targetPanel.group && sourcePanel.group.panels.length === 1) {
        splitTab(tabId, preview.direction as WorkspaceSplitDirection)
        clearTabDrop()
        return true
      }

      sourcePanel.api.moveTo({
        group: targetPanel.group,
        position:
          preview.direction === "above"
            ? "top"
            : preview.direction === "below"
              ? "bottom"
              : preview.direction,
      })
      sourcePanel.api.setActive()
      clearTabDrop()
      return true
    },
    [clearTabDrop, dockApi, previewTabDrop, splitTab, dropPreviewRef]
  )

  return { splitTab, clearTabDrop, previewTabDrop, commitTabDrop }
}
