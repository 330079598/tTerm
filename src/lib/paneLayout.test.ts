import { describe, expect, it } from "vitest"

import { dragSash, getSplitAt, layoutPaneTree, minimumExtent } from "@/lib/paneLayout"
import { paneLeaf, type PaneNode } from "@/lib/tabGroups"

const tree: PaneNode = {
  type: "split",
  orientation: "horizontal",
  sizes: [0.5, 0.5],
  children: [
    paneLeaf("a"),
    {
      type: "split",
      orientation: "vertical",
      sizes: [0.25, 0.75],
      children: [paneLeaf("b"), paneLeaf("c")],
    },
  ],
}

const area = { left: 0, top: 0, width: 1005, height: 405 }

describe("layoutPaneTree", () => {
  it("fills the area with the panes, the gap between them", () => {
    const { panes } = layoutPaneTree(tree, area, 5)
    expect(panes.get("a")).toEqual({ left: 0, top: 0, width: 500, height: 405 })
    expect(panes.get("b")).toEqual({ left: 505, top: 0, width: 500, height: 100 })
    expect(panes.get("c")).toEqual({ left: 505, top: 105, width: 500, height: 300 })
  })

  it("puts a sash in each gap, with the split it resizes", () => {
    const { sashes } = layoutPaneTree(tree, area, 5)
    expect(sashes).toEqual([
      {
        left: 500,
        top: 0,
        width: 5,
        height: 405,
        path: [],
        index: 0,
        orientation: "horizontal",
        room: 1000,
      },
      {
        left: 505,
        top: 100,
        width: 500,
        height: 5,
        path: [1],
        index: 0,
        orientation: "vertical",
        room: 400,
      },
    ])
  })

  it("rounds edges to whole pixels so panes meet without a seam", () => {
    const thirds: PaneNode = {
      type: "split",
      orientation: "horizontal",
      sizes: [1 / 3, 1 / 3, 1 / 3],
      children: [paneLeaf("a"), paneLeaf("b"), paneLeaf("c")],
    }
    const { panes } = layoutPaneTree(thirds, { left: 0, top: 0, width: 100, height: 10 }, 0)
    const boxes = ["a", "b", "c"].map((id) => panes.get(id)!)
    expect(boxes.map((box) => box.left)).toEqual([0, 33, 67])
    expect(boxes.reduce((sum, box) => sum + box.width, 0)).toBe(100)
  })
})

describe("minimumExtent", () => {
  it("adds up panes along a split and takes the widest across one", () => {
    const minimum = { width: 200, height: 100 }
    expect(minimumExtent(tree, "horizontal", minimum, 5)).toBe(405)
    expect(minimumExtent(tree, "vertical", minimum, 5)).toBe(205)
  })
})

describe("dragSash", () => {
  it("moves the room between the two panes beside the sash", () => {
    expect(dragSash([0.5, 0.5], 0, 100, 1000, [200, 200])).toEqual([0.6, 0.4])
  })

  it("stops where a pane would drop under its minimum", () => {
    expect(dragSash([0.5, 0.5], 0, 400, 1000, [200, 200])).toEqual([0.8, 0.2])
    expect(dragSash([0.5, 0.5], 0, -400, 1000, [200, 200])).toEqual([0.2, 0.8])
  })

  it("leaves a pane already under its minimum where it is", () => {
    expect(dragSash([0.1, 0.9], 0, -50, 1000, [200, 200])).toEqual([0.1, 0.9])
  })
})

describe("getSplitAt", () => {
  it("follows the path to a split, and finds no split at a pane", () => {
    expect(getSplitAt(tree, [1])?.orientation).toBe("vertical")
    expect(getSplitAt(tree, [0])).toBeUndefined()
  })
})
