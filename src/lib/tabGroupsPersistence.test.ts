import { describe, expect, it } from "vitest"

import { getPaneIds, paneLeaf, type PaneNode, type TabGroup } from "@/lib/tabGroups"
import { restoreTabGroups, serializeTabGroups } from "@/lib/tabGroupsPersistence"

const row = (children: PaneNode[], sizes = children.map(() => 1 / children.length)): PaneNode => ({
  type: "split",
  orientation: "horizontal",
  children,
  sizes,
})

describe("serializeTabGroups", () => {
  it("leaves out panes whose tabs are not saved, and groups left empty", () => {
    const groups: TabGroup[] = [
      { id: "g1", root: row([paneLeaf("a"), paneLeaf("settings")]), activeTabId: "settings" },
      { id: "g2", root: paneLeaf("editor"), activeTabId: "editor" },
    ]
    expect(serializeTabGroups(groups, new Set(["a"]))).toEqual({
      version: 2,
      groups: [{ root: paneLeaf("a"), activeTabId: "a" }],
    })
  })

  it("keeps a zoom on a saved pane", () => {
    const groups: TabGroup[] = [
      { id: "g1", root: row([paneLeaf("a"), paneLeaf("b")]), activeTabId: "b", zoomedTabId: "b" },
    ]
    expect(serializeTabGroups(groups, new Set(["a", "b"])).groups[0].zoomedTabId).toBe("b")
  })
})

describe("restoreTabGroups", () => {
  it("reads back what was saved, for the tabs that came back", () => {
    const saved = serializeTabGroups(
      [
        { id: "g1", root: row([paneLeaf("a"), paneLeaf("b")]), activeTabId: "b" },
        { id: "g2", root: paneLeaf("c"), activeTabId: "c" },
      ],
      new Set(["a", "b", "c"])
    )
    const restored = restoreTabGroups(JSON.parse(JSON.stringify(saved)), ["a", "c"])
    expect(restored).toEqual([
      { root: paneLeaf("a"), activeTabId: "a", zoomedTabId: undefined },
      { root: paneLeaf("c"), activeTabId: "c", zoomedTabId: undefined },
    ])
  })

  it("skips nodes that are not a pane tree", () => {
    const layout = {
      version: 2,
      groups: [
        { root: { type: "split", orientation: "diagonal", children: [], sizes: [] } },
        { root: { type: "pane", tabId: 7 } },
        { root: { type: "pane", tabId: "a" }, activeTabId: "a" },
        "nonsense",
      ],
    }
    expect(restoreTabGroups(layout, ["a"])).toEqual([
      { root: paneLeaf("a"), activeTabId: "a", zoomedTabId: undefined },
    ])
  })

  it("restores nothing from a missing or broken layout", () => {
    expect(restoreTabGroups(null, ["a"])).toEqual([])
    expect(restoreTabGroups("layout", ["a"])).toEqual([])
    expect(restoreTabGroups({ grid: {} }, ["a"])).toEqual([])
  })

  it("turns a Dockview grid into one split of the tabs in front, the rest tabs of their own", () => {
    const dockview = {
      grid: {
        orientation: "HORIZONTAL",
        root: {
          type: "branch",
          data: [
            { type: "leaf", data: { id: "1", views: ["a", "b"], activeView: "b" }, size: 300 },
            {
              type: "branch",
              size: 700,
              data: [
                { type: "leaf", data: { id: "2", views: ["c"], activeView: "c" }, size: 100 },
                { type: "leaf", data: { id: "3", views: ["d"], activeView: "d" }, size: 300 },
              ],
            },
          ],
        },
      },
      panels: {},
      activeGroup: "3",
    }
    const restored = restoreTabGroups(dockview, ["a", "b", "c", "d"])
    expect(restored.map((group) => getPaneIds(group.root))).toEqual([["a"], ["b", "c", "d"]])
    const split = restored[1]
    expect(split.activeTabId).toBe("d")
    expect(split.root).toEqual({
      type: "split",
      orientation: "horizontal",
      sizes: [0.3, 0.7],
      children: [
        paneLeaf("b"),
        {
          type: "split",
          orientation: "vertical",
          sizes: [0.25, 0.75],
          children: [paneLeaf("c"), paneLeaf("d")],
        },
      ],
    })
  })
})
