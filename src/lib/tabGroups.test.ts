import { describe, expect, it } from "vitest"

import {
  breakOutPane,
  focusPaneInGroups,
  getFallbackTabId,
  getPaneIds,
  getVisiblePaneIds,
  insertNextTo,
  mergeGroup,
  moveGroup,
  normalizePaneTree,
  paneLeaf,
  placePane,
  reconcileGroups,
  removePanes,
  removeTabsFromGroups,
  resizeSplit,
  toggleZoom,
  ungroupPanes,
  type PaneNode,
  type TabGroup,
} from "@/lib/tabGroups"

const row = (children: PaneNode[], sizes = children.map(() => 1 / children.length)): PaneNode => ({
  type: "split",
  orientation: "horizontal",
  children,
  sizes,
})
const column = (
  children: PaneNode[],
  sizes = children.map(() => 1 / children.length)
): PaneNode => ({
  type: "split",
  orientation: "vertical",
  children,
  sizes,
})
const pane = paneLeaf

function group(id: string, root: PaneNode, activeTabId = getPaneIds(root)[0]): TabGroup {
  return { id, root, activeTabId }
}

function counter() {
  let count = 0
  return () => `new-${++count}`
}

describe("insertNextTo", () => {
  it("splits a lone pane in two", () => {
    expect(insertNextTo(pane("a"), "a", pane("b"), "right")).toEqual(row([pane("a"), pane("b")]))
    expect(insertNextTo(pane("a"), "a", pane("b"), "above")).toEqual(column([pane("b"), pane("a")]))
  })

  it("joins a split of the same orientation, taking half of the target's room", () => {
    const tree = insertNextTo(row([pane("a"), pane("b")]), "b", pane("c"), "right")
    expect(tree).toEqual(row([pane("a"), pane("b"), pane("c")], [0.5, 0.25, 0.25]))
  })

  it("nests a split across the orientation", () => {
    const tree = insertNextTo(row([pane("a"), pane("b")]), "b", pane("c"), "below")
    expect(tree).toEqual(row([pane("a"), column([pane("b"), pane("c")])]))
  })

  it("flattens an inserted tree that runs the same way", () => {
    const tree = insertNextTo(pane("a"), "a", row([pane("b"), pane("c")]), "right")
    expect(tree).toEqual(row([pane("a"), pane("b"), pane("c")], [0.5, 0.25, 0.25]))
  })
})

describe("removePanes", () => {
  it("gives the room of a removed pane to the others, in proportion", () => {
    const tree = removePanes(row([pane("a"), pane("b"), pane("c")], [0.5, 0.25, 0.25]), ["c"])
    expect(tree).toEqual(row([pane("a"), pane("b")], [2 / 3, 1 / 3]))
  })

  it("collapses a split left with one child and joins it to its parent", () => {
    const tree = row([pane("a"), column([pane("b"), row([pane("c"), pane("d")])])])
    expect(removePanes(tree, ["b"])).toEqual(
      row([pane("a"), pane("c"), pane("d")], [0.5, 0.25, 0.25])
    )
  })

  it("returns null once no pane is left", () => {
    expect(removePanes(row([pane("a"), pane("b")]), ["a", "b"])).toBeNull()
  })
})

describe("normalizePaneTree", () => {
  it("scales sizes that do not sum to one", () => {
    expect(normalizePaneTree(row([pane("a"), pane("b")], [3, 1]))).toEqual(
      row([pane("a"), pane("b")], [0.75, 0.25])
    )
  })

  it("shares the room out evenly when no size is usable", () => {
    expect(normalizePaneTree(row([pane("a"), pane("b")], [0, -1]))).toEqual(
      row([pane("a"), pane("b")], [0.5, 0.5])
    )
  })
})

describe("resizeSplit", () => {
  it("sets the sizes of the split at the path", () => {
    const tree = row([pane("a"), column([pane("b"), pane("c")])])
    expect(resizeSplit(tree, [1], [0.25, 0.75])).toEqual(
      row([pane("a"), column([pane("b"), pane("c")], [0.25, 0.75])])
    )
  })

  it("ignores sizes that do not match the split", () => {
    const tree = row([pane("a"), pane("b")])
    expect(resizeSplit(tree, [], [1])).toBe(tree)
  })
})

describe("removeTabsFromGroups", () => {
  it("drops emptied groups and moves a group's focus to the nearest pane", () => {
    const groups = [
      group("g1", row([pane("a"), pane("b"), pane("c")]), "b"),
      group("g2", pane("d")),
    ]
    const result = removeTabsFromGroups(groups, ["b", "d"])
    expect(result).toEqual([
      { id: "g1", root: row([pane("a"), pane("c")]), activeTabId: "c", zoomedTabId: undefined },
    ])
  })

  it("ends the zoom of a removed pane", () => {
    const groups = [{ ...group("g1", row([pane("a"), pane("b")])), zoomedTabId: "b" }]
    expect(removeTabsFromGroups(groups, ["b"])[0].zoomedTabId).toBeUndefined()
  })

  it("returns the same groups when none holds the tabs", () => {
    const groups = [group("g1", pane("a"))]
    expect(removeTabsFromGroups(groups, ["x"])).toBe(groups)
  })
})

describe("getFallbackTabId", () => {
  const before = [
    group("g1", pane("a")),
    group("g2", row([pane("b"), pane("c")]), "c"),
    group("g3", pane("d")),
  ]

  it("stays in the group when another of its panes is left", () => {
    expect(getFallbackTabId(before, removeTabsFromGroups(before, ["c"]), "c")).toBe("b")
  })

  it("moves to the group that takes the closed group's place", () => {
    expect(getFallbackTabId(before, removeTabsFromGroups(before, ["b", "c"]), "c")).toBe("d")
  })

  it("moves to the last group once the closed one was last", () => {
    expect(getFallbackTabId(before, removeTabsFromGroups(before, ["d"]), "d")).toBe("c")
  })
})

describe("reconcileGroups", () => {
  it("gives tabs no group holds a group of their own and drops panes of closed tabs", () => {
    const result = reconcileGroups(
      [group("g1", row([pane("a"), pane("gone")]))],
      ["a", "b"],
      counter()
    )
    expect(result.map((candidate) => [candidate.id, getPaneIds(candidate.root)])).toEqual([
      ["g1", ["a"]],
      ["new-1", ["b"]],
    ])
  })

  it("keeps a tab listed in two groups in only one", () => {
    const result = reconcileGroups(
      [group("g1", pane("a")), group("g2", pane("a"))],
      ["a"],
      counter()
    )
    expect(result.flatMap((candidate) => getPaneIds(candidate.root))).toEqual(["a"])
  })

  it("returns the same groups when they already match the tabs", () => {
    const groups = [group("g1", pane("a"))]
    expect(reconcileGroups(groups, ["a"], counter())).toBe(groups)
  })
})

describe("moving panes", () => {
  const groups = [group("g1", row([pane("a"), pane("b")])), group("g2", pane("c"))]

  it("places a pane beside another, leaving its own group if need be", () => {
    const result = placePane(groups, "c", "a", "below")
    expect(result).toHaveLength(1)
    expect(result[0].root).toEqual(row([column([pane("a"), pane("c")]), pane("b")]))
    expect(result[0].activeTabId).toBe("c")
  })

  it("merges a whole group beside a pane of another", () => {
    const result = mergeGroup(groups, "c", "b", "right")
    expect(result.map((candidate) => getPaneIds(candidate.root))).toEqual([["a", "b", "c"]])
  })

  it("does not merge a group into itself", () => {
    expect(mergeGroup(groups, "a", "b", "right")).toBe(groups)
  })

  it("breaks a pane out into a group right after its own, or at an index", () => {
    expect(breakOutPane(groups, "b", "g3").map((candidate) => candidate.id)).toEqual([
      "g1",
      "g3",
      "g2",
    ])
    expect(breakOutPane(groups, "b", "g3", 0).map((candidate) => candidate.id)).toEqual([
      "g3",
      "g1",
      "g2",
    ])
    expect(breakOutPane(groups, "c", "g3")).toBe(groups)
  })

  it("ungroups a split into one group per pane, in place", () => {
    const result = ungroupPanes(
      [group("g0", pane("x")), group("g1", row([pane("a"), pane("b")]), "b")],
      "a",
      counter()
    )
    expect(result.map((candidate) => [candidate.id, getPaneIds(candidate.root)])).toEqual([
      ["g0", ["x"]],
      ["new-1", ["a"]],
      ["g1", ["b"]],
    ])
  })

  it("reorders groups", () => {
    expect(moveGroup(groups, 1, 0).map((candidate) => candidate.id)).toEqual(["g2", "g1"])
    expect(moveGroup(groups, 0, 5)).toBe(groups)
  })
})

describe("zoom", () => {
  const groups = [group("g1", row([pane("a"), pane("b")]))]

  it("zooms a pane and restores it on the second toggle", () => {
    const zoomed = toggleZoom(groups, "b")
    expect(zoomed[0]).toMatchObject({ zoomedTabId: "b", activeTabId: "b" })
    expect(getVisiblePaneIds(zoomed, "b")).toEqual(["b"])
    expect(toggleZoom(zoomed, "b")[0].zoomedTabId).toBeUndefined()
  })

  it("has nothing to zoom in a lone pane", () => {
    const lone = [group("g1", pane("a"))]
    expect(toggleZoom(lone, "a")).toBe(lone)
  })

  it("ends the zoom when another pane of the group is focused", () => {
    const zoomed = toggleZoom(groups, "b")
    expect(focusPaneInGroups(zoomed, "a")[0]).toMatchObject({
      activeTabId: "a",
      zoomedTabId: undefined,
    })
    expect(focusPaneInGroups(zoomed, "b")).toBe(zoomed)
  })

  it("shows every pane of the active group when none is zoomed", () => {
    expect(getVisiblePaneIds(groups, "a")).toEqual(["a", "b"])
    expect(getVisiblePaneIds(groups, null)).toEqual([])
  })
})
