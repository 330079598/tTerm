// Where a tab's panes and the dividers between them sit, in pixels, and how
// dragging a divider shares the room out again.

import type { PaneNode, SplitOrientation } from "@/lib/tabGroups"

export interface PaneBox {
  left: number
  top: number
  width: number
  height: number
}

/** The divider between a split's children `index` and `index + 1`. */
export interface SashBox extends PaneBox {
  /** The split's child index at each level from the root. */
  path: number[]
  index: number
  orientation: SplitOrientation
  /** The split's length along its orientation, less the gaps: what its sizes are shares of. */
  room: number
}

export interface PaneLayout {
  panes: Map<string, PaneBox>
  sashes: SashBox[]
}

export interface PaneMinimum {
  width: number
  height: number
}

function along(orientation: SplitOrientation, box: PaneBox) {
  return orientation === "horizontal" ? box.width : box.height
}

/**
 * Lays a tree out in `box`, `gap` pixels apart. Edges are rounded to whole
 * pixels, so neighbouring panes neither overlap nor leave a seam.
 */
export function layoutPaneTree(root: PaneNode, box: PaneBox, gap: number): PaneLayout {
  const layout: PaneLayout = { panes: new Map(), sashes: [] }
  const visit = (node: PaneNode, area: PaneBox, path: number[]) => {
    if (node.type === "pane") {
      layout.panes.set(node.tabId, area)
      return
    }
    const horizontal = node.orientation === "horizontal"
    const start = horizontal ? area.left : area.top
    const room = Math.max(0, along(node.orientation, area) - gap * (node.children.length - 1))
    let share = 0
    let edge = start
    node.children.forEach((child, index) => {
      share += node.sizes[index]
      const isLast = index === node.children.length - 1
      const end = isLast
        ? start + along(node.orientation, area)
        : Math.round(start + room * share + gap * index)
      const childArea = horizontal
        ? { left: edge, top: area.top, width: end - edge, height: area.height }
        : { left: area.left, top: edge, width: area.width, height: end - edge }
      visit(child, childArea, [...path, index])
      if (!isLast) {
        layout.sashes.push({
          ...(horizontal
            ? { left: end, top: area.top, width: gap, height: area.height }
            : { left: area.left, top: end, width: area.width, height: gap }),
          path,
          index,
          orientation: node.orientation,
          room,
        })
      }
      edge = end + gap
    })
  }
  visit(root, box, [])
  return layout
}

/** The least room a tree needs along `orientation` for none of its panes to drop under the minimum. */
export function minimumExtent(
  node: PaneNode,
  orientation: SplitOrientation,
  minimum: PaneMinimum,
  gap: number
): number {
  if (node.type === "pane") {
    return orientation === "horizontal" ? minimum.width : minimum.height
  }
  const extents = node.children.map((child) => minimumExtent(child, orientation, minimum, gap))
  return node.orientation === orientation
    ? extents.reduce((sum, extent) => sum + extent, 0) + gap * (node.children.length - 1)
    : Math.max(...extents)
}

/**
 * The split's sizes once the divider after child `index` has moved `delta`
 * pixels. Only the two children beside it change, and neither shrinks under
 * its minimum, unless it was already under it.
 */
export function dragSash(
  sizes: readonly number[],
  index: number,
  delta: number,
  room: number,
  minimums: readonly number[]
): number[] {
  if (room <= 0 || index < 0 || index + 1 >= sizes.length) return [...sizes]
  const before = sizes[index] * room
  const after = sizes[index + 1] * room
  const lowest = -Math.max(0, before - minimums[index])
  const highest = Math.max(0, after - minimums[index + 1])
  const moved = Math.min(Math.max(delta, lowest), highest)
  const next = [...sizes]
  next[index] = (before + moved) / room
  next[index + 1] = (after - moved) / room
  return next
}

/** The split node found by following `path` from the root. */
export function getSplitAt(
  root: PaneNode,
  path: readonly number[]
): Extract<PaneNode, { type: "split" }> | undefined {
  let node: PaneNode | undefined = root
  for (const index of path) {
    node = node?.type === "split" ? node.children[index] : undefined
  }
  return node?.type === "split" ? node : undefined
}
