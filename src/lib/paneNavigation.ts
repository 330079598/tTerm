/** Picking the split pane a pane shortcut moves the focus to. */

export type PaneDirection = "left" | "right" | "up" | "down"

/** A pane's id and where it sits on screen, in client pixels. */
export interface PaneRect {
  id: string
  left: number
  top: number
  right: number
  bottom: number
}

// Pane edges are rounded to whole pixels, so panes in one row can differ by a
// pixel in their tops.
const EDGE_TOLERANCE = 2

function isVisible(pane: PaneRect) {
  return pane.right > pane.left && pane.bottom > pane.top
}

/**
 * Orders panes as they read on screen: top to bottom, then left to right.
 * Panes hidden behind a maximized pane have no size and are left out.
 */
export function sortPanesByReadingOrder(panes: PaneRect[]): PaneRect[] {
  return panes
    .filter(isVisible)
    .sort((a, b) => (Math.abs(a.top - b.top) > EDGE_TOLERANCE ? a.top - b.top : a.left - b.left))
}

/**
 * Returns the pane `offset` steps away from the active one in reading order,
 * wrapping around both ends, or undefined when no other pane is in sight.
 */
export function getSiblingPaneId(
  panes: PaneRect[],
  activeId: string | undefined,
  offset: number
): string | undefined {
  const ordered = sortPanesByReadingOrder(panes)
  const index = ordered.findIndex((pane) => pane.id === activeId)
  if (index === -1 || ordered.length < 2) {
    return undefined
  }
  return ordered[(index + offset + ordered.length) % ordered.length].id
}

/** How far `pane` lies beyond `active` in `direction`, or null when it is not on that side. */
function distanceBeyond(active: PaneRect, pane: PaneRect, direction: PaneDirection) {
  const distance = {
    left: active.left - pane.right,
    right: pane.left - active.right,
    up: active.top - pane.bottom,
    down: pane.top - active.bottom,
  }[direction]
  return distance >= -EDGE_TOLERANCE ? distance : null
}

/** How much of `active`'s side facing `direction` the pane shares. */
function sharedEdge(active: PaneRect, pane: PaneRect, direction: PaneDirection) {
  return direction === "left" || direction === "right"
    ? Math.min(active.bottom, pane.bottom) - Math.max(active.top, pane.top)
    : Math.min(active.right, pane.right) - Math.max(active.left, pane.left)
}

/**
 * Returns the pane next to the active one in `direction`, without wrapping.
 * Of the nearest panes on that side, the one sharing most of the active
 * pane's edge wins, then the one used most recently, as `recentIds` lists
 * pane ids from most to least recent.
 */
export function findPaneInDirection(
  panes: PaneRect[],
  activeId: string | undefined,
  direction: PaneDirection,
  recentIds: string[] = []
): string | undefined {
  const visible = panes.filter(isVisible)
  const active = visible.find((pane) => pane.id === activeId)
  if (!active) {
    return undefined
  }

  const candidates = visible.flatMap((pane) => {
    if (pane.id === active.id) return []
    const distance = distanceBeyond(active, pane, direction)
    const shared = sharedEdge(active, pane, direction)
    return distance === null || shared <= 0 ? [] : [{ id: pane.id, distance, shared }]
  })
  if (candidates.length === 0) {
    return undefined
  }

  const nearest = Math.min(...candidates.map((candidate) => candidate.distance))
  const recency = (id: string) => {
    const index = recentIds.indexOf(id)
    return index === -1 ? Number.POSITIVE_INFINITY : index
  }
  return candidates
    .filter((candidate) => candidate.distance - nearest <= EDGE_TOLERANCE)
    .sort((a, b) => b.shared - a.shared || recency(a.id) - recency(b.id))[0].id
}
