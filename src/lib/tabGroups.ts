// The tabs in the tab bar. Each holds one or more panes, laid out by a split
// tree, and each pane shows one of the app's tabs: a terminal, a connection
// or a page. A tab with a single pane looks as tabs always did.

export type SplitDirection = "left" | "right" | "above" | "below"

/** `horizontal` lays the children side by side, `vertical` stacks them. */
export type SplitOrientation = "horizontal" | "vertical"

export type PaneNode =
  | { type: "pane"; tabId: string }
  | {
      type: "split"
      orientation: SplitOrientation
      children: PaneNode[]
      /** Each child's share of the split, summing to 1. */
      sizes: number[]
    }

export interface TabGroup {
  id: string
  root: PaneNode
  /** The pane focused last, focused again when the group is. */
  activeTabId: string
  /** A pane filling the whole group while the split behind it stays as it was. */
  zoomedTabId?: string
}

export function paneLeaf(tabId: string): PaneNode {
  return { type: "pane", tabId }
}

export function splitOrientation(direction: SplitDirection): SplitOrientation {
  return direction === "left" || direction === "right" ? "horizontal" : "vertical"
}

/** The tab ids of a tree's panes, left to right and top to bottom. */
export function getPaneIds(node: PaneNode): string[] {
  return node.type === "pane" ? [node.tabId] : node.children.flatMap(getPaneIds)
}

export function findGroupOfTab(groups: readonly TabGroup[], tabId: string): TabGroup | undefined {
  return groups.find((group) => getPaneIds(group.root).includes(tabId))
}

function scaleToOne(sizes: number[]): number[] {
  const total = sizes.reduce((sum, size) => sum + (size > 0 ? size : 0), 0)
  if (total <= 0) return sizes.map(() => 1 / sizes.length)
  return sizes.map((size) => (size > 0 ? size : 0) / total)
}

/**
 * Puts a tree into its one canonical shape: a split of one child is that
 * child, a split inside a split of the same orientation joins its parent,
 * and the sizes of every split sum to 1.
 */
export function normalizePaneTree(node: PaneNode): PaneNode {
  if (node.type === "pane") return node
  const children: PaneNode[] = []
  const sizes: number[] = []
  const ownSizes = scaleToOne(
    node.children.map((_, index) => node.sizes[index] ?? 1 / node.children.length)
  )
  node.children.forEach((child, index) => {
    const normalized = normalizePaneTree(child)
    if (normalized.type === "split" && normalized.orientation === node.orientation) {
      children.push(...normalized.children)
      sizes.push(...normalized.sizes.map((size) => size * ownSizes[index]))
    } else {
      children.push(normalized)
      sizes.push(ownSizes[index])
    }
  })
  if (children.length === 1) return children[0]
  return { type: "split", orientation: node.orientation, children, sizes: scaleToOne(sizes) }
}

function insertInto(
  node: PaneNode,
  targetTabId: string,
  inserted: PaneNode,
  direction: SplitDirection
): PaneNode {
  const orientation = splitOrientation(direction)
  const before = direction === "left" || direction === "above"
  if (node.type === "pane") {
    if (node.tabId !== targetTabId) return node
    return {
      type: "split",
      orientation,
      children: before ? [inserted, node] : [node, inserted],
      sizes: [0.5, 0.5],
    }
  }
  const index = node.children.findIndex(
    (child) => child.type === "pane" && child.tabId === targetTabId
  )
  if (index >= 0 && node.orientation === orientation) {
    // The new pane takes half of the one it splits, the rest keep their sizes.
    const half = node.sizes[index] / 2
    const at = before ? index : index + 1
    const children = [...node.children]
    children.splice(at, 0, inserted)
    const sizes = [...node.sizes]
    sizes[index] = half
    sizes.splice(at, 0, half)
    return { ...node, children, sizes }
  }
  return {
    ...node,
    children: node.children.map((child) => insertInto(child, targetTabId, inserted, direction)),
  }
}

/** Puts a pane, or a whole tree, beside the target pane, which gives up half its room. */
export function insertNextTo(
  root: PaneNode,
  targetTabId: string,
  inserted: PaneNode,
  direction: SplitDirection
): PaneNode {
  return normalizePaneTree(insertInto(root, targetTabId, inserted, direction))
}

function removeFrom(node: PaneNode, tabIds: ReadonlySet<string>): PaneNode | null {
  if (node.type === "pane") return tabIds.has(node.tabId) ? null : node
  const children: PaneNode[] = []
  const sizes: number[] = []
  node.children.forEach((child, index) => {
    const kept = removeFrom(child, tabIds)
    if (kept) {
      children.push(kept)
      sizes.push(node.sizes[index])
    }
  })
  if (children.length === 0) return null
  // The panes left share the room of the ones that went, in proportion.
  return { ...node, children, sizes: scaleToOne(sizes) }
}

/** The tree without the given panes, or null when none is left. */
export function removePanes(root: PaneNode, tabIds: Iterable<string>): PaneNode | null {
  const removed = removeFrom(root, new Set(tabIds))
  return removed ? normalizePaneTree(removed) : null
}

/** Sets the sizes of the split found by following `path`, a child index per level. */
export function resizeSplit(root: PaneNode, path: readonly number[], sizes: number[]): PaneNode {
  if (root.type === "pane") return root
  if (path.length === 0) {
    return root.children.length === sizes.length ? { ...root, sizes: scaleToOne(sizes) } : root
  }
  const [index, ...rest] = path
  const child = root.children[index]
  if (!child) return root
  const children = [...root.children]
  children[index] = resizeSplit(child, rest, sizes)
  return { ...root, children }
}

// The pane in `remaining` closest to `tabId` in the old order, after it first.
function nearestPane(order: string[], tabId: string, remaining: string[]): string {
  const index = order.indexOf(tabId)
  const kept = new Set(remaining)
  for (let offset = 1; offset < order.length; offset++) {
    const after = order[index + offset]
    if (after !== undefined && kept.has(after)) return after
    const before = order[index - offset]
    if (before !== undefined && kept.has(before)) return before
  }
  return remaining[0]
}

/** The groups without the given panes; a group left with no pane goes. */
export function removeTabsFromGroups(groups: TabGroup[], tabIds: Iterable<string>): TabGroup[] {
  const ids = new Set(tabIds)
  if (!groups.some((group) => getPaneIds(group.root).some((id) => ids.has(id)))) return groups
  return groups.flatMap((group) => {
    const order = getPaneIds(group.root)
    if (!order.some((id) => ids.has(id))) return [group]
    const root = removePanes(group.root, ids)
    if (!root) return []
    const remaining = getPaneIds(root)
    return [
      {
        ...group,
        root,
        activeTabId: remaining.includes(group.activeTabId)
          ? group.activeTabId
          : nearestPane(order, group.activeTabId, remaining),
        zoomedTabId:
          group.zoomedTabId && remaining.includes(group.zoomedTabId)
            ? group.zoomedTabId
            : undefined,
      },
    ]
  })
}

/**
 * The pane to focus once the active one is gone: another pane of its group,
 * or else the group that took its group's place in the bar, or the last one.
 */
export function getFallbackTabId(
  previous: readonly TabGroup[],
  next: readonly TabGroup[],
  activeTabId: string | null
): string | undefined {
  if (next.length === 0) return undefined
  const previousGroup = activeTabId ? findGroupOfTab(previous, activeTabId) : undefined
  if (!previousGroup) return undefined
  const survivor = next.find((group) => group.id === previousGroup.id)
  if (survivor) return survivor.activeTabId
  // Groups after the closed one move up into its place.
  const index = previous.indexOf(previousGroup)
  const after = previous.slice(index + 1).find((group) => next.some(({ id }) => id === group.id))
  if (after) return after.activeTabId
  return next[next.length - 1].activeTabId
}

/**
 * Makes every tab sit in exactly one group: panes of tabs that are gone are
 * dropped, and tabs no group holds get a group of their own at the end.
 */
export function reconcileGroups(
  groups: TabGroup[],
  tabIds: readonly string[],
  createGroupId: () => string
): TabGroup[] {
  const known = new Set(tabIds)
  const seen = new Set<string>()
  const stale: string[] = []
  for (const group of groups) {
    for (const id of getPaneIds(group.root)) {
      if (!known.has(id) || seen.has(id)) stale.push(id)
      seen.add(id)
    }
  }
  let result = stale.length > 0 ? removeTabsFromGroups(groups, stale) : groups
  // A pane listed twice was removed from both places above; bring it back once.
  const placed = new Set(result.flatMap((group) => getPaneIds(group.root)))
  const missing = tabIds.filter((id) => !placed.has(id))
  if (missing.length > 0) {
    result = [
      ...result,
      ...missing.map((tabId) => ({
        id: createGroupId(),
        root: paneLeaf(tabId),
        activeTabId: tabId,
      })),
    ]
  }
  return result
}

/** Makes `tabId` its group's active pane; focusing a pane hidden by a zoom ends the zoom. */
export function focusPaneInGroups(groups: TabGroup[], tabId: string): TabGroup[] {
  const group = findGroupOfTab(groups, tabId)
  if (!group) return groups
  const zoomedTabId = group.zoomedTabId === tabId ? group.zoomedTabId : undefined
  if (group.activeTabId === tabId && group.zoomedTabId === zoomedTabId) return groups
  return groups.map((candidate) =>
    candidate === group ? { ...group, activeTabId: tabId, zoomedTabId } : candidate
  )
}

/**
 * Moves a pane next to another one, out of its own group if need be. A group
 * the move leaves empty goes.
 */
export function placePane(
  groups: TabGroup[],
  tabId: string,
  targetTabId: string,
  direction: SplitDirection
): TabGroup[] {
  if (tabId === targetTabId || !findGroupOfTab(groups, targetTabId)) return groups
  const without = removeTabsFromGroups(groups, [tabId])
  return without.map((group) =>
    getPaneIds(group.root).includes(targetTabId)
      ? {
          ...group,
          root: insertNextTo(group.root, targetTabId, paneLeaf(tabId), direction),
          activeTabId: tabId,
          zoomedTabId: undefined,
        }
      : group
  )
}

/** Moves every pane of the group holding `tabId` next to `targetTabId`, splits and all. */
export function mergeGroup(
  groups: TabGroup[],
  tabId: string,
  targetTabId: string,
  direction: SplitDirection
): TabGroup[] {
  const source = findGroupOfTab(groups, tabId)
  const target = findGroupOfTab(groups, targetTabId)
  if (!source || !target || source === target) return groups
  return groups
    .filter((group) => group !== source)
    .map((group) =>
      group === target
        ? {
            ...group,
            root: insertNextTo(group.root, targetTabId, source.root, direction),
            activeTabId: source.activeTabId,
            zoomedTabId: undefined,
          }
        : group
    )
}

/** Takes a pane out of its group into a group of its own, right after it or at `index`. */
export function breakOutPane(
  groups: TabGroup[],
  tabId: string,
  newGroupId: string,
  index?: number
): TabGroup[] {
  const source = findGroupOfTab(groups, tabId)
  if (!source || getPaneIds(source.root).length < 2) return groups
  const without = removeTabsFromGroups(groups, [tabId])
  const at = index ?? without.findIndex((group) => group.id === source.id) + 1
  const result = [...without]
  result.splice(Math.min(Math.max(at, 0), result.length), 0, {
    id: newGroupId,
    root: paneLeaf(tabId),
    activeTabId: tabId,
  })
  return result
}

/** Gives every pane of the group holding `tabId` a group of its own, in their order. */
export function ungroupPanes(
  groups: TabGroup[],
  tabId: string,
  createGroupId: () => string
): TabGroup[] {
  const source = findGroupOfTab(groups, tabId)
  if (!source || source.root.type === "pane") return groups
  const index = groups.indexOf(source)
  const singles = getPaneIds(source.root).map((paneId) => ({
    id: paneId === source.activeTabId ? source.id : createGroupId(),
    root: paneLeaf(paneId),
    activeTabId: paneId,
  }))
  return [...groups.slice(0, index), ...singles, ...groups.slice(index + 1)]
}

/** Zooms the pane, or ends the zoom when it is the zoomed one; a lone pane has nothing to zoom over. */
export function toggleZoom(groups: TabGroup[], tabId: string): TabGroup[] {
  const group = findGroupOfTab(groups, tabId)
  if (!group || group.root.type === "pane") return groups
  return groups.map((candidate) =>
    candidate === group
      ? {
          ...group,
          activeTabId: tabId,
          zoomedTabId: group.zoomedTabId === tabId ? undefined : tabId,
        }
      : candidate
  )
}

export function moveGroup(groups: TabGroup[], fromIndex: number, toIndex: number): TabGroup[] {
  if (
    fromIndex < 0 ||
    fromIndex >= groups.length ||
    toIndex < 0 ||
    toIndex >= groups.length ||
    fromIndex === toIndex
  ) {
    return groups
  }
  const result = [...groups]
  const [moved] = result.splice(fromIndex, 1)
  result.splice(toIndex, 0, moved)
  return result
}

/** The panes in sight: those of the active tab's group, or only its zoomed one. */
export function getVisiblePaneIds(
  groups: readonly TabGroup[],
  activeTabId: string | null
): string[] {
  const group = activeTabId ? findGroupOfTab(groups, activeTabId) : undefined
  if (!group) return []
  return group.zoomedTabId ? [group.zoomedTabId] : getPaneIds(group.root)
}
