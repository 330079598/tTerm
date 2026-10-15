import { describe, expect, it } from "vitest"

import {
  findPaneInDirection,
  getSiblingPaneId,
  sortPanesByReadingOrder,
  type PaneRect,
} from "@/lib/paneNavigation"

function pane(id: string, left: number, top: number, right: number, bottom: number): PaneRect {
  return { id, left, top, right, bottom }
}

// One tall pane on the left, two stacked on the right, 8px gaps between them.
//  ┌───┬───┐
//  │   │ b │
//  │ a ├───┤
//  │   │ c │
//  └───┴───┘
const tallLeft = [
  pane("c", 508, 304, 1000, 600),
  pane("a", 0, 0, 500, 600),
  pane("b", 508, 0, 1000, 296),
]

describe("sortPanesByReadingOrder", () => {
  it("orders panes top to bottom, then left to right", () => {
    expect(sortPanesByReadingOrder(tallLeft).map((p) => p.id)).toEqual(["a", "b", "c"])
  })

  it("treats tops a pixel apart as one row", () => {
    const panes = [pane("right", 504, 0, 1000, 600), pane("left", 0, 1, 496, 600)]
    expect(sortPanesByReadingOrder(panes).map((p) => p.id)).toEqual(["left", "right"])
  })

  it("leaves out panes hidden behind a maximized pane", () => {
    const panes = [pane("shown", 0, 0, 1000, 600), pane("hidden", 0, 0, 0, 0)]
    expect(sortPanesByReadingOrder(panes).map((p) => p.id)).toEqual(["shown"])
  })
})

describe("getSiblingPaneId", () => {
  it("steps through panes in reading order and wraps around", () => {
    expect(getSiblingPaneId(tallLeft, "a", 1)).toBe("b")
    expect(getSiblingPaneId(tallLeft, "c", 1)).toBe("a")
    expect(getSiblingPaneId(tallLeft, "a", -1)).toBe("c")
  })

  it("finds no sibling with a single pane in sight", () => {
    expect(getSiblingPaneId([pane("only", 0, 0, 100, 100)], "only", 1)).toBeUndefined()
  })

  it("finds no sibling when the active pane is unknown", () => {
    expect(getSiblingPaneId(tallLeft, "missing", 1)).toBeUndefined()
    expect(getSiblingPaneId(tallLeft, undefined, 1)).toBeUndefined()
  })
})

describe("findPaneInDirection", () => {
  it("moves to the pane beside the active one", () => {
    expect(findPaneInDirection(tallLeft, "b", "left")).toBe("a")
    expect(findPaneInDirection(tallLeft, "b", "down")).toBe("c")
    expect(findPaneInDirection(tallLeft, "c", "up")).toBe("b")
  })

  it("does not wrap at the edge of the workspace", () => {
    expect(findPaneInDirection(tallLeft, "a", "left")).toBeUndefined()
    expect(findPaneInDirection(tallLeft, "b", "up")).toBeUndefined()
  })

  it("skips panes that share no edge with the active one", () => {
    // b and c are both right of a, but neither is below or above it.
    expect(findPaneInDirection(tallLeft, "a", "down")).toBeUndefined()
  })

  it("prefers the most recently used pane when neighbours share the edge equally", () => {
    expect(findPaneInDirection(tallLeft, "a", "right", ["c", "a", "b"])).toBe("c")
    expect(findPaneInDirection(tallLeft, "a", "right", ["b", "c"])).toBe("b")
  })

  it("prefers the neighbour sharing more of the edge over a more recent one", () => {
    const panes = [
      pane("a", 0, 0, 500, 600),
      pane("big", 508, 0, 1000, 400),
      pane("small", 508, 408, 1000, 600),
    ]
    expect(findPaneInDirection(panes, "a", "right", ["small"])).toBe("big")
  })

  it("prefers the nearest pane over one further along", () => {
    const panes = [
      pane("a", 0, 0, 300, 600),
      pane("near", 308, 0, 600, 600),
      pane("far", 608, 0, 1000, 600),
    ]
    expect(findPaneInDirection(panes, "a", "right", ["far"])).toBe("near")
  })

  it("ignores panes hidden behind a maximized pane", () => {
    const panes = [pane("a", 0, 0, 1000, 600), pane("hidden", 0, 0, 0, 0)]
    expect(findPaneInDirection(panes, "a", "right")).toBeUndefined()
  })
})
