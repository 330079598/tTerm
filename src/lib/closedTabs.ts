import { fillShape, shapeOfTree, type LayoutShape } from "@/lib/paneTemplates"
import {
  findGroupOfTab,
  getPaneIds,
  insertNextTo,
  paneLeaf,
  removePanes,
  type PaneNode,
  type SplitDirection,
  type TabGroup,
} from "@/lib/tabGroups"
import { isSessionTab, type Tab } from "@/types/tab"

/** How many closings can be undone, the most recent first. */
export const CLOSED_TABS_LIMIT = 20

/** A closed session tab, as it is opened again. */
export type ClosedTab = Omit<Tab, "id" | "isActive">

/** The pane a closed pane goes back beside, and on which side of it. */
export interface PaneAnchor {
  tabId: string
  direction: SplitDirection
}

/** Where closed tabs go back. */
export type ClosedLayout =
  /** A whole tab of the bar: its split, the pane that had the focus, its place in the bar. */
  | { kind: "group"; shape: LayoutShape; activeIndex: number; index: number }
  /** One pane of a tab that stayed open, with that tab's split as it was. */
  | { kind: "pane"; closedId: string; previousRoot: PaneNode; anchor: PaneAnchor | null }

/** One closing that can be undone: the tabs it closed and where they were. */
export interface ClosedEntry {
  tabs: ClosedTab[]
  layout: ClosedLayout
}

/**
 * What opening `tab` again needs, or null for a page or a remote file. A
 * local shell starts again in the directory it was in, a connection connects
 * again; passwords typed for it are not kept once it is closed.
 */
export function closedTabSnapshot(tab: Tab): ClosedTab | null {
  if (!isSessionTab(tab)) return null
  const {
    id: _id,
    isActive: _isActive,
    pid: _pid,
    hasConnected: _hasConnected,
    sessionNonce: _sessionNonce,
    quickConnectAuth: _quickConnectAuth,
    ...kept
  } = tab
  const connection = tab.connection && {
    ...tab.connection,
    password: undefined,
    privateKeyPassphrase: undefined,
    jumpHosts: tab.connection.jumpHosts?.map((jump) => ({
      ...jump,
      password: undefined,
      privateKeyPassphrase: undefined,
    })),
  }
  // A quick connection opened again is not offered for saving again.
  return { ...kept, quickConnect: false, connection }
}

/**
 * The pane a closed pane goes back beside: the one before it in its split,
 * so it goes back after it, else the one after it.
 */
export function paneAnchor(root: PaneNode, tabId: string): PaneAnchor | null {
  if (root.type === "pane") return null
  const index = root.children.findIndex((child) => child.type === "pane" && child.tabId === tabId)
  if (index < 0) {
    for (const child of root.children) {
      const anchor = paneAnchor(child, tabId)
      if (anchor) return anchor
    }
    return null
  }
  const horizontal = root.orientation === "horizontal"
  if (index > 0) {
    const before = getPaneIds(root.children[index - 1])
    return { tabId: before[before.length - 1], direction: horizontal ? "right" : "below" }
  }
  const after = root.children[1]
  if (!after) return null
  return { tabId: getPaneIds(after)[0], direction: horizontal ? "left" : "above" }
}

// A whole tab of the bar as closed. Its pages and remote files are not
// opened again, so its split closes up around them.
function closedGroupEntry(
  group: TabGroup,
  index: number,
  byId: ReadonlyMap<string, Tab>
): ClosedEntry | null {
  const snapshots = new Map<string, ClosedTab>()
  for (const id of getPaneIds(group.root)) {
    const tab = byId.get(id)
    const snapshot = tab && closedTabSnapshot(tab)
    if (snapshot) snapshots.set(id, snapshot)
  }
  const dropped = getPaneIds(group.root).filter((id) => !snapshots.has(id))
  const root = removePanes(group.root, dropped)
  if (!root) return null
  const order = getPaneIds(root)
  return {
    tabs: order.flatMap((id) => snapshots.get(id) ?? []),
    layout: {
      kind: "group",
      shape: shapeOfTree(root),
      activeIndex: Math.max(order.indexOf(group.activeTabId), 0),
      index,
    },
  }
}

/**
 * The closings that closing the tabs `ids` makes, in the order of the bar:
 * one for each tab of the bar closed with all its panes, and one for each
 * pane closed out of a tab that stays open.
 */
export function closedEntries(
  tabs: readonly Tab[],
  groups: readonly TabGroup[],
  ids: readonly string[]
): ClosedEntry[] {
  const closing = new Set(ids)
  const byId = new Map(tabs.map((tab) => [tab.id, tab]))
  const entries: ClosedEntry[] = []
  groups.forEach((group, index) => {
    const paneIds = getPaneIds(group.root)
    const closed = paneIds.filter((id) => closing.has(id))
    if (closed.length === 0) return
    if (closed.length === paneIds.length) {
      const entry = closedGroupEntry(group, index, byId)
      if (entry) entries.push(entry)
      return
    }
    for (const id of closed) {
      const tab = byId.get(id)
      const snapshot = tab && closedTabSnapshot(tab)
      if (!snapshot) continue
      entries.push({
        tabs: [snapshot],
        layout: {
          kind: "pane",
          closedId: id,
          previousRoot: group.root,
          anchor: paneAnchor(group.root, id),
        },
      })
    }
  })
  return entries
}

/** The closings with `entries` made last, the oldest dropped past the limit. */
export function pushClosedEntries(
  closed: readonly ClosedEntry[],
  entries: readonly ClosedEntry[]
): ClosedEntry[] {
  return [...closed, ...entries].slice(-CLOSED_TABS_LIMIT)
}

function renamePane(node: PaneNode, from: string, to: string): PaneNode {
  if (node.type === "pane") return node.tabId === from ? paneLeaf(to) : node
  return { ...node, children: node.children.map((child) => renamePane(child, from, to)) }
}

function sameTree(a: PaneNode | null, b: PaneNode): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

/**
 * The groups with tabs opened again, given the ids `ids`, put back as
 * `layout` says. A whole tab of the bar goes back to its place with its
 * split. A pane goes back into its tab: exactly where it was while that tab
 * is as it was left, else beside the pane it was next to. With that pane
 * gone too, it is left out, to get a tab of the bar of its own.
 */
export function placeReopenedTabs(
  groups: TabGroup[],
  layout: ClosedLayout,
  ids: readonly string[],
  createGroupId: () => string
): TabGroup[] {
  if (ids.length === 0) return groups
  if (layout.kind === "group") {
    const group: TabGroup = {
      id: createGroupId(),
      root: fillShape(layout.shape, ids),
      activeTabId: ids[layout.activeIndex] ?? ids[0],
    }
    const at = Math.min(Math.max(layout.index, 0), groups.length)
    return [...groups.slice(0, at), group, ...groups.slice(at)]
  }
  const tabId = ids[0]
  const anchor = layout.anchor
  const target = anchor && findGroupOfTab(groups, anchor.tabId)
  if (!anchor || !target) return groups
  const previous = renamePane(layout.previousRoot, layout.closedId, tabId)
  const root = sameTree(removePanes(previous, [tabId]), target.root)
    ? previous
    : insertNextTo(target.root, anchor.tabId, paneLeaf(tabId), anchor.direction)
  return groups.map((group) =>
    group === target ? { ...group, root, activeTabId: tabId, zoomedTabId: undefined } : group
  )
}

/** The pane to focus once `layout`'s tabs, given `ids`, are open again. */
export function reopenedActiveTabId(layout: ClosedLayout, ids: readonly string[]): string {
  return layout.kind === "group" ? (ids[layout.activeIndex] ?? ids[0]) : ids[0]
}
