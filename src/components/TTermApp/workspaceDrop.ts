import type { WorkspaceSplitDirection } from "@/components/TTermApp/workspaceContext"

export type WorkspaceDropDirection = "left" | "right" | "above" | "below" | "center"

export type WorkspaceDropPreview = {
  direction: WorkspaceDropDirection
  sourceTabId: string
  targetTabId: string
  top: number
  left: number
  width: number
  height: number
}

export type PendingSplit = {
  direction: WorkspaceSplitDirection
  sourceTabId: string
  tabId: string
}
