import type { PaneBox } from "@/lib/paneLayout"
import type { SplitDirection } from "@/lib/tabGroups"

/** Where a dragged tab or pane would land: beside a pane of the tab in sight. */
export type WorkspaceDropPreview = PaneBox & {
  direction: SplitDirection
  sourceTabId: string
  targetTabId: string
}

export interface WorkspaceDropTarget {
  tabId: string
  box: PaneBox
  direction: SplitDirection
}

/**
 * The pane under a point, in the workspace's own coordinates, and the side of
 * it nearest the point, which is where a drop splits it.
 */
export function findDropTarget(
  boxes: ReadonlyMap<string, PaneBox>,
  x: number,
  y: number
): WorkspaceDropTarget | null {
  for (const [tabId, box] of boxes) {
    if (box.width <= 0 || box.height <= 0) continue
    if (x < box.left || x > box.left + box.width || y < box.top || y > box.top + box.height) {
      continue
    }
    const offsetX = (x - box.left) / box.width - 0.5
    const offsetY = (y - box.top) / box.height - 0.5
    const direction: SplitDirection =
      Math.abs(offsetX) > Math.abs(offsetY)
        ? offsetX < 0
          ? "left"
          : "right"
        : offsetY < 0
          ? "above"
          : "below"
    return { tabId, box, direction }
  }
  return null
}

/** Where among the tab bar's tabs a pane dropped at `clientX` goes: before the first tab whose middle it is left of. */
export function getTabBarDropIndex(tabCenters: readonly number[], clientX: number): number {
  const index = tabCenters.findIndex((center) => clientX < center)
  return index === -1 ? tabCenters.length : index
}
