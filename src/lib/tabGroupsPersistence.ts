// The tabs' split layouts as the session saves them, and reading them back:
// the current format, and the Dockview layout earlier versions saved.

import {
  getPaneIds,
  normalizePaneTree,
  paneLeaf,
  removePanes,
  type PaneNode,
  type SplitOrientation,
  type TabGroup,
} from "@/lib/tabGroups"

export const TAB_GROUPS_LAYOUT_VERSION = 2

export interface SavedTabGroup {
  root: PaneNode
  activeTabId: string
  zoomedTabId?: string
}

export interface SavedTabGroups {
  version: typeof TAB_GROUPS_LAYOUT_VERSION
  groups: SavedTabGroup[]
}

/** The groups to save, holding only the panes whose tabs are saved too. */
export function serializeTabGroups(
  groups: readonly TabGroup[],
  savedTabIds: ReadonlySet<string>
): SavedTabGroups {
  const saved = groups.flatMap((group) => {
    const unsaved = getPaneIds(group.root).filter((id) => !savedTabIds.has(id))
    const root = unsaved.length > 0 ? removePanes(group.root, unsaved) : group.root
    if (!root) return []
    const paneIds = getPaneIds(root)
    return [
      {
        root,
        activeTabId: paneIds.includes(group.activeTabId) ? group.activeTabId : paneIds[0],
        ...(group.zoomedTabId && paneIds.includes(group.zoomedTabId)
          ? { zoomedTabId: group.zoomedTabId }
          : {}),
      },
    ]
  })
  return { version: TAB_GROUPS_LAYOUT_VERSION, groups: saved }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

// A saved tree is checked node by node: a session file may come from another
// version, or have been edited by hand.
function readPaneNode(value: unknown): PaneNode | null {
  if (!isRecord(value)) return null
  if (value.type === "pane") {
    return typeof value.tabId === "string" ? paneLeaf(value.tabId) : null
  }
  if (value.type !== "split") return null
  if (value.orientation !== "horizontal" && value.orientation !== "vertical") return null
  if (!Array.isArray(value.children) || !Array.isArray(value.sizes)) return null
  const children = value.children.map(readPaneNode)
  if (children.length === 0 || children.some((child) => child === null)) return null
  const sizes = value.sizes.map((size) => (typeof size === "number" && size > 0 ? size : 0))
  return {
    type: "split",
    orientation: value.orientation,
    children: children as PaneNode[],
    sizes: children.map((_, index) => sizes[index] ?? 0),
  }
}

function readSavedGroups(value: Record<string, unknown>): SavedTabGroup[] {
  if (!Array.isArray(value.groups)) return []
  return value.groups.flatMap((group) => {
    if (!isRecord(group)) return []
    const root = readPaneNode(group.root)
    if (!root) return []
    return [
      {
        root,
        activeTabId: typeof group.activeTabId === "string" ? group.activeTabId : "",
        zoomedTabId: typeof group.zoomedTabId === "string" ? group.zoomedTabId : undefined,
      },
    ]
  })
}

// Dockview kept one grid of groups for the whole window, each group stacking
// tabs. Its grid becomes one tab's split, each group showing the tab it had in
// front; the tabs behind them become tabs of their own.
function readDockviewLayout(value: Record<string, unknown>): SavedTabGroup[] {
  const grid = value.grid
  if (!isRecord(grid)) return []
  const behind: string[] = []
  const frontByGroupId = new Map<string, string>()
  const visit = (node: unknown, orientation: SplitOrientation): PaneNode | null => {
    if (!isRecord(node)) return null
    if (node.type === "leaf") {
      const data = isRecord(node.data) ? node.data : {}
      const views = Array.isArray(data.views)
        ? data.views.filter((view): view is string => typeof view === "string")
        : []
      const front =
        typeof data.activeView === "string" && views.includes(data.activeView)
          ? data.activeView
          : views[0]
      if (!front) return null
      if (typeof data.id === "string") frontByGroupId.set(data.id, front)
      behind.push(...views.filter((view) => view !== front))
      return paneLeaf(front)
    }
    if (node.type !== "branch" || !Array.isArray(node.data)) return null
    const crossed: SplitOrientation = orientation === "horizontal" ? "vertical" : "horizontal"
    const entries = node.data.flatMap((child) => {
      const pane = visit(child, crossed)
      const size = isRecord(child) && typeof child.size === "number" ? child.size : 1
      return pane ? [{ pane, size }] : []
    })
    if (entries.length === 0) return null
    return {
      type: "split",
      orientation,
      children: entries.map((entry) => entry.pane),
      sizes: entries.map((entry) => entry.size),
    }
  }
  const root = visit(grid.root, grid.orientation === "VERTICAL" ? "vertical" : "horizontal")
  if (!root) return []
  const front = normalizePaneTree(root)
  const activeFront =
    typeof value.activeGroup === "string" ? frontByGroupId.get(value.activeGroup) : undefined
  return [
    { root: front, activeTabId: activeFront ?? getPaneIds(front)[0] },
    ...behind.map((tabId) => ({ root: paneLeaf(tabId), activeTabId: tabId })),
  ]
}

/**
 * The groups of a saved layout, for the tabs restored with it. Panes whose
 * tab was not restored are left out; tabs no group holds are left for the
 * caller to give groups of their own. A layout saved by Dockview keeps the
 * tabs' order from the tab list, since its groups had none of their own.
 */
export function restoreTabGroups(
  layout: unknown,
  tabIds: readonly string[]
): Omit<TabGroup, "id">[] {
  if (!isRecord(layout)) return []
  const isDockview = layout.version !== TAB_GROUPS_LAYOUT_VERSION
  const saved = isDockview ? readDockviewLayout(layout) : readSavedGroups(layout)
  const known = new Set(tabIds)
  const seen = new Set<string>()
  const groups = saved.flatMap((group) => {
    const dropped = getPaneIds(group.root).filter((id) => !known.has(id) || seen.has(id))
    const root = dropped.length > 0 ? removePanes(group.root, dropped) : group.root
    if (!root) return []
    const paneIds = getPaneIds(root)
    paneIds.forEach((id) => seen.add(id))
    return [
      {
        root: normalizePaneTree(root),
        activeTabId: paneIds.includes(group.activeTabId) ? group.activeTabId : paneIds[0],
        zoomedTabId:
          group.zoomedTabId && paneIds.includes(group.zoomedTabId) ? group.zoomedTabId : undefined,
      },
    ]
  })
  if (!isDockview) return groups
  const position = (group: Omit<TabGroup, "id">) =>
    Math.min(...getPaneIds(group.root).map((id) => tabIds.indexOf(id)))
  return [...groups].sort((first, second) => position(first) - position(second))
}
