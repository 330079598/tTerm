import { closedEntries, type ClosedLayout, type ClosedTab } from "@/lib/closedTabs"
import type { LayoutShape } from "@/lib/paneTemplates"
import { getPaneIds, type TabGroup } from "@/lib/tabGroups"
import { isSessionTab, type Tab } from "@/types/tab"

/**
 * One tab of the bar put away: the sessions of its panes, its split, and the
 * pane that had the focus. Opening it again connects each session anew.
 */
export interface CollapsedGroup {
  id: string
  tabs: ClosedTab[]
  /**
   * For each of `tabs`, the key its terminal's content is kept under, shown
   * again above its new session; null for a terminal that showed nothing.
   */
  contentKeys: (string | null)[]
  shape: LayoutShape
  activeIndex: number
}

/** The tabs of the bar put away by one click, kept until they are opened or deleted. */
export interface CollapsedSet {
  id: string
  /** Milliseconds since the Unix epoch. */
  collapsedAt: number
  groups: CollapsedGroup[]
}

/** The tabs of the bar that hold a session, the ones collapsing all of them puts away. */
export function collapsibleGroupIds(tabs: readonly Tab[], groups: readonly TabGroup[]): string[] {
  const sessions = new Set(tabs.filter((tab) => isSessionTab(tab)).map((tab) => tab.id))
  return groups
    .filter((group) => getPaneIds(group.root).some((id) => sessions.has(id)))
    .map((group) => group.id)
}

/** How a set is made: new ids, and the key a pane's terminal content is kept under. */
export interface CollapseOptions {
  now: number
  createId: () => string
  /** Keeps what the terminal of `tabId` shows; returns its key, or null when it shows nothing. */
  keepContent: (tabId: string) => string | null
}

/**
 * The set that putting away the tabs of the bar `groupIds` makes, or null
 * when none of them holds a session. Pages and remote files in their splits
 * are not kept, as closing a tab does not keep them either.
 */
export function collapsedSetOf(
  tabs: readonly Tab[],
  groups: readonly TabGroup[],
  groupIds: readonly string[],
  { now, createId, keepContent }: CollapseOptions
): CollapsedSet | null {
  const wanted = new Set(groupIds)
  const byId = new Map(tabs.map((tab) => [tab.id, tab]))
  const collapsed = groups
    .filter((group) => wanted.has(group.id))
    .flatMap((group): CollapsedGroup[] => {
      const paneIds = getPaneIds(group.root)
      const [entry] = closedEntries(tabs, [group], paneIds)
      if (entry?.layout.kind !== "group" || entry.tabs.length === 0) return []
      // The entry keeps the sessions in the order of the split, pages left out.
      const sessionIds = paneIds.filter((id) => isSessionTab(byId.get(id)))
      return [
        {
          id: createId(),
          tabs: entry.tabs,
          contentKeys: sessionIds.map(keepContent),
          shape: entry.layout.shape,
          activeIndex: entry.layout.activeIndex,
        },
      ]
    })
  if (collapsed.length === 0) return null
  return { id: createId(), collapsedAt: now, groups: collapsed }
}

/** Where a collapsed tab of the bar goes when it opens again: at the end of the bar. */
export function collapsedGroupLayout(group: CollapsedGroup): ClosedLayout {
  return {
    kind: "group",
    shape: group.shape,
    activeIndex: group.activeIndex,
    index: Number.MAX_SAFE_INTEGER,
  }
}

/** The sets with `set` added, the newest first as the panel lists them. */
export function addCollapsedSet(sets: readonly CollapsedSet[], set: CollapsedSet): CollapsedSet[] {
  return [set, ...sets]
}

/** The sets without the set `setId`. */
export function removeCollapsedSet(sets: readonly CollapsedSet[], setId: string): CollapsedSet[] {
  return sets.filter((set) => set.id !== setId)
}

/** The sets without one tab of the bar; a set left empty goes too. */
export function removeCollapsedGroup(
  sets: readonly CollapsedSet[],
  setId: string,
  groupId: string
): CollapsedSet[] {
  return sets.flatMap((set) => {
    if (set.id !== setId) return [set]
    const groups = set.groups.filter((group) => group.id !== groupId)
    return groups.length > 0 ? [{ ...set, groups }] : []
  })
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function isShape(value: unknown): value is LayoutShape {
  if (!isRecord(value)) return false
  if (value.type === "pane") return true
  return (
    value.type === "split" &&
    (value.orientation === "horizontal" || value.orientation === "vertical") &&
    Array.isArray(value.children) &&
    value.children.length > 0 &&
    value.children.every(isShape) &&
    Array.isArray(value.sizes) &&
    value.sizes.every((size) => typeof size === "number")
  )
}

function countPanes(shape: LayoutShape): number {
  return shape.type === "pane"
    ? 1
    : shape.children.reduce((sum, child) => sum + countPanes(child), 0)
}

// A saved tab of the bar is kept only when its split still has a pane for
// each of its sessions, so opening it can lay them out as they were.
function parseGroup(value: unknown): CollapsedGroup | null {
  if (!isRecord(value) || typeof value.id !== "string") return null
  if (!Array.isArray(value.tabs) || !isShape(value.shape)) return null
  const tabs = value.tabs.filter(
    (tab): tab is ClosedTab => isRecord(tab) && isSessionTab(tab as Pick<Tab, "type">)
  )
  if (tabs.length === 0 || tabs.length !== countPanes(value.shape)) return null
  const keys = Array.isArray(value.contentKeys) ? value.contentKeys : []
  const contentKeys = tabs.map((_, index) =>
    typeof keys[index] === "string" ? (keys[index] as string) : null
  )
  const activeIndex =
    typeof value.activeIndex === "number" && value.activeIndex < tabs.length
      ? Math.max(0, Math.floor(value.activeIndex))
      : 0
  return { id: value.id, tabs, contentKeys, shape: value.shape, activeIndex }
}

/** The sets read back from storage, leaving out whatever no longer makes sense. */
export function parseCollapsedSets(value: unknown): CollapsedSet[] {
  if (!Array.isArray(value)) return []
  return value.flatMap((entry) => {
    if (!isRecord(entry) || typeof entry.id !== "string") return []
    if (typeof entry.collapsedAt !== "number" || !Array.isArray(entry.groups)) return []
    const groups = entry.groups.flatMap((group) => parseGroup(group) ?? [])
    return groups.length > 0 ? [{ id: entry.id, collapsedAt: entry.collapsedAt, groups }] : []
  })
}

/** How many sessions the sets hold, for the badge on the button. */
export function countCollapsedTabs(sets: readonly CollapsedSet[]): number {
  return sets.reduce(
    (sum, set) => sum + set.groups.reduce((inner, group) => inner + group.tabs.length, 0),
    0
  )
}
