// Layout templates: the shape of a split tab, without what runs in its panes,
// saved to lay another tab out the same way.

import {
  findGroupOfTab,
  getPaneIds,
  normalizePaneTree,
  paneLeaf,
  type PaneNode,
  type SplitOrientation,
  type TabGroup,
} from "@/lib/tabGroups"

export type LayoutShape =
  | { type: "pane" }
  | {
      type: "split"
      orientation: SplitOrientation
      children: LayoutShape[]
      /** Each child's share of the split, summing to 1. */
      sizes: number[]
    }

export interface PaneLayoutTemplate {
  id: string
  name: string
  root: LayoutShape
}

/** No template holds more panes than this, so a bad one can't open dozens of sessions. */
export const MAX_TEMPLATE_PANES = 9
const MAX_TEMPLATE_DEPTH = 4
const MAX_SAVED_TEMPLATES = 30

const pane: LayoutShape = { type: "pane" }

function split(orientation: SplitOrientation, children: LayoutShape[], sizes?: number[]) {
  return {
    type: "split" as const,
    orientation,
    children,
    sizes: sizes ?? children.map(() => 1 / children.length),
  }
}

/** The templates tTerm comes with; their names are translated by `labelKey`. */
export const BUILTIN_PANE_TEMPLATES: (PaneLayoutTemplate & { labelKey: string })[] = [
  {
    id: "builtin:columns-2",
    labelKey: "columns2",
    name: "",
    root: split("horizontal", [pane, pane]),
  },
  { id: "builtin:rows-2", labelKey: "rows2", name: "", root: split("vertical", [pane, pane]) },
  {
    id: "builtin:columns-3",
    labelKey: "columns3",
    name: "",
    root: split("horizontal", [pane, pane, pane]),
  },
  {
    id: "builtin:main-right-2",
    labelKey: "mainRight2",
    name: "",
    root: split("horizontal", [pane, split("vertical", [pane, pane])], [0.6, 0.4]),
  },
  {
    id: "builtin:main-bottom-2",
    labelKey: "mainBottom2",
    name: "",
    root: split("vertical", [pane, split("horizontal", [pane, pane])], [0.6, 0.4]),
  },
  {
    id: "builtin:grid-4",
    labelKey: "grid4",
    name: "",
    root: split("horizontal", [split("vertical", [pane, pane]), split("vertical", [pane, pane])]),
  },
]

export function isBuiltinTemplateId(id: string): boolean {
  return id.startsWith("builtin:")
}

export function countTemplatePanes(shape: LayoutShape): number {
  return shape.type === "pane"
    ? 1
    : shape.children.reduce((sum, child) => sum + countTemplatePanes(child), 0)
}

/** The shape of a tab's split tree, its panes left out. */
export function shapeOfTree(node: PaneNode): LayoutShape {
  if (node.type === "pane") return { type: "pane" }
  return {
    type: "split",
    orientation: node.orientation,
    children: node.children.map(shapeOfTree),
    sizes: [...node.sizes],
  }
}

/** Puts `tabIds` into the shape's panes, left to right and top to bottom. */
export function fillShape(shape: LayoutShape, tabIds: readonly string[]): PaneNode {
  let next = 0
  const fill = (node: LayoutShape): PaneNode =>
    node.type === "pane"
      ? paneLeaf(tabIds[next++])
      : {
          type: "split",
          orientation: node.orientation,
          children: node.children.map(fill),
          sizes: [...node.sizes],
        }
  return normalizePaneTree(fill(shape))
}

function readShape(value: unknown, depth: number): LayoutShape | null {
  if (typeof value !== "object" || value === null) return null
  const node = value as Record<string, unknown>
  if (node.type === "pane") return { type: "pane" }
  if (node.type !== "split" || depth >= MAX_TEMPLATE_DEPTH) return null
  if (node.orientation !== "horizontal" && node.orientation !== "vertical") return null
  if (!Array.isArray(node.children) || node.children.length < 2) return null
  const children = node.children.map((child) => readShape(child, depth + 1))
  if (children.some((child) => child === null)) return null
  const rawSizes = Array.isArray(node.sizes) ? node.sizes : []
  const sizes = children.map((_, index) => {
    const size = rawSizes[index]
    return typeof size === "number" && Number.isFinite(size) && size > 0 ? size : 0
  })
  const total = sizes.reduce((sum, size) => sum + size, 0)
  return {
    type: "split",
    orientation: node.orientation,
    children: children as LayoutShape[],
    sizes: total > 0 ? sizes.map((size) => size / total) : sizes.map(() => 1 / sizes.length),
  }
}

/** A saved shape, or null when it is malformed, too deep or has too many panes. */
export function normalizeLayoutShape(value: unknown): LayoutShape | null {
  const shape = readShape(value, 0)
  if (!shape || shape.type === "pane") return null
  return countTemplatePanes(shape) <= MAX_TEMPLATE_PANES ? shape : null
}

/** The saved templates from a config, dropping the broken ones and repeated ids. */
export function normalizePaneTemplates(value: unknown): PaneLayoutTemplate[] {
  if (!Array.isArray(value)) return []
  const seen = new Set<string>()
  const templates: PaneLayoutTemplate[] = []
  for (const entry of value) {
    if (typeof entry !== "object" || entry === null) continue
    const { id, name, root } = entry as Record<string, unknown>
    if (typeof id !== "string" || !id || isBuiltinTemplateId(id) || seen.has(id)) continue
    const shape = normalizeLayoutShape(root)
    if (!shape) continue
    seen.add(id)
    templates.push({ id, name: typeof name === "string" ? name.trim() : "", root: shape })
    if (templates.length >= MAX_SAVED_TEMPLATES) break
  }
  return templates
}

/**
 * Lays the group holding `tabId` out as `shape`. Its panes fill the shape in
 * their order, `newTabIds` fill the panes left over, and panes the shape has
 * no room for move out into tabs of their own right after it.
 */
export function applyTemplateToGroups(
  groups: TabGroup[],
  tabId: string,
  shape: LayoutShape,
  newTabIds: readonly string[],
  createGroupId: () => string
): TabGroup[] {
  const group = findGroupOfTab(groups, tabId)
  if (!group) return groups
  const slots = countTemplatePanes(shape)
  const existing = getPaneIds(group.root)
  // The active pane always stays, even when it is not among the first ones.
  const kept = existing.includes(tabId)
    ? [tabId, ...existing.filter((id) => id !== tabId)]
        .slice(0, slots)
        .sort((a, b) => existing.indexOf(a) - existing.indexOf(b))
    : existing.slice(0, slots)
  const filled = [...kept, ...newTabIds].slice(0, slots)
  if (filled.length < slots) return groups
  const leftOver = existing.filter((id) => !kept.includes(id))
  const index = groups.indexOf(group)
  const laidOut: TabGroup = {
    ...group,
    root: fillShape(shape, filled),
    activeTabId: tabId,
    zoomedTabId: undefined,
  }
  const singles = leftOver.map((id) => ({
    id: createGroupId(),
    root: paneLeaf(id),
    activeTabId: id,
  }))
  return [...groups.slice(0, index), laidOut, ...singles, ...groups.slice(index + 1)]
}
