import { describe, expect, it } from "vitest"

import {
  addCollapsedSet,
  collapsedGroupLayout,
  collapsedSetOf,
  collapsibleGroupIds,
  countCollapsedTabs,
  parseCollapsedSets,
  removeCollapsedGroup,
  removeCollapsedSet,
  type CollapsedSet,
} from "@/lib/collapsedTabs"
import { paneLeaf, type TabGroup } from "@/lib/tabGroups"
import type { Tab } from "@/types/tab"

const tab = (id: string, overrides: Partial<Tab> = {}): Tab => ({
  id,
  title: id,
  type: "terminal",
  isActive: false,
  sessionNonce: 2,
  pid: 7,
  connection: { type: "terminal", cwd: `/work/${id}` },
  ...overrides,
})

const single = (id: string): TabGroup => ({ id: `g-${id}`, root: paneLeaf(id), activeTabId: id })

const split: TabGroup = {
  id: "g-split",
  root: {
    type: "split",
    orientation: "horizontal",
    children: [paneLeaf("b"), paneLeaf("c")],
    sizes: [0.5, 0.5],
  },
  activeTabId: "c",
}

function options(now = 1, keepContent: (tabId: string) => string | null = () => null) {
  let next = 0
  return { now, createId: () => `id-${++next}`, keepContent }
}

describe("collapsibleGroupIds", () => {
  it("leaves out tabs of the bar holding only pages", () => {
    const tabs = [tab("a"), tab("s", { type: "settings", connection: undefined })]
    expect(collapsibleGroupIds(tabs, [single("a"), single("s")])).toEqual(["g-a"])
  })
})

describe("collapsedSetOf", () => {
  it("keeps each tab of the bar with its split and focused pane", () => {
    const tabs = [tab("a"), tab("b", { type: "ssh" }), tab("c")]
    const set = collapsedSetOf(tabs, [single("a"), split], ["g-a", "g-split"], options(100))
    expect(set?.collapsedAt).toBe(100)
    expect(set?.groups).toHaveLength(2)
    const [first, second] = set!.groups
    expect(first.tabs.map((saved) => saved.title)).toEqual(["a"])
    expect(first.shape).toEqual({ type: "pane" })
    expect(second.tabs.map((saved) => saved.title)).toEqual(["b", "c"])
    expect(second.shape.type).toBe("split")
    expect(second.activeIndex).toBe(1)
    expect(second.tabs[0]).not.toHaveProperty("pid")
    expect(second.tabs[0]).not.toHaveProperty("id")
  })

  it("keeps a local tab's directory so it starts there again", () => {
    const set = collapsedSetOf([tab("a")], [single("a")], ["g-a"], options())
    expect(set?.groups[0].tabs[0].connection?.cwd).toBe("/work/a")
  })

  it("only takes the tabs of the bar asked for", () => {
    const tabs = [tab("a"), tab("b"), tab("c")]
    const set = collapsedSetOf(tabs, [single("a"), split], ["g-split"], options())
    expect(set?.groups.map((group) => group.tabs.length)).toEqual([2])
  })

  it("is null when nothing holds a session", () => {
    const tabs = [tab("s", { type: "settings", connection: undefined })]
    expect(collapsedSetOf(tabs, [single("s")], ["g-s"], options())).toBeNull()
  })

  it("keeps each pane's terminal content under a key, in the order of the split", () => {
    const tabs = [tab("b"), tab("c"), tab("s", { type: "settings", connection: undefined })]
    const withPage: TabGroup = {
      ...split,
      root: {
        type: "split",
        orientation: "horizontal",
        children: [paneLeaf("b"), paneLeaf("s"), paneLeaf("c")],
        sizes: [0.3, 0.3, 0.4],
      },
    }
    const keepContent = (id: string) => (id === "c" ? `key-${id}` : null)
    const set = collapsedSetOf(tabs, [withPage], ["g-split"], options(1, keepContent))
    expect(set?.groups[0].tabs.map((saved) => saved.title)).toEqual(["b", "c"])
    expect(set?.groups[0].contentKeys).toEqual([null, "key-c"])
  })
})

describe("editing the sets", () => {
  const set = (id: string, groupIds: string[]): CollapsedSet => ({
    id,
    collapsedAt: 1,
    groups: groupIds.map((groupId) => ({
      id: groupId,
      tabs: [{ title: groupId, type: "terminal" }],
      contentKeys: [null],
      shape: { type: "pane" },
      activeIndex: 0,
    })),
  })

  it("puts the newest set first", () => {
    const sets = addCollapsedSet([set("old", ["x"])], set("new", ["y"]))
    expect(sets.map((entry) => entry.id)).toEqual(["new", "old"])
  })

  it("drops a set once its last tab is gone", () => {
    const sets = [set("one", ["x", "y"]), set("two", ["z"])]
    const fewer = removeCollapsedGroup(sets, "one", "x")
    expect(fewer[0].groups.map((group) => group.id)).toEqual(["y"])
    expect(removeCollapsedGroup(sets, "two", "z").map((entry) => entry.id)).toEqual(["one"])
    expect(removeCollapsedSet(sets, "one").map((entry) => entry.id)).toEqual(["two"])
  })

  it("counts the sessions held", () => {
    expect(countCollapsedTabs([set("one", ["x", "y"]), set("two", ["z"])])).toBe(3)
  })

  it("puts a tab of the bar back at the end", () => {
    const layout = collapsedGroupLayout(set("one", ["x"]).groups[0])
    expect(layout).toMatchObject({ kind: "group", activeIndex: 0 })
    expect(layout.kind === "group" && layout.index).toBe(Number.MAX_SAFE_INTEGER)
  })
})

describe("parseCollapsedSets", () => {
  const group = {
    id: "g",
    tabs: [{ title: "a", type: "terminal" }],
    contentKeys: ["k"],
    shape: { type: "pane" },
    activeIndex: 0,
  }

  it("reads back what was saved", () => {
    const saved = [{ id: "s", collapsedAt: 5, groups: [group] }]
    expect(parseCollapsedSets(saved)).toEqual(saved)
  })

  it("drops what no longer makes sense", () => {
    expect(parseCollapsedSets(null)).toEqual([])
    expect(parseCollapsedSets([{ id: "s", groups: [group] }])).toEqual([])
    const mismatched = { ...group, shape: { type: "split", orientation: "horizontal" } }
    const page = { ...group, tabs: [{ title: "s", type: "settings" }] }
    expect(parseCollapsedSets([{ id: "s", collapsedAt: 1, groups: [mismatched, page] }])).toEqual(
      []
    )
  })

  it("reads groups saved without content as having none", () => {
    const { contentKeys: _keys, ...older } = group
    const saved = [{ id: "s", collapsedAt: 1, groups: [older] }]
    expect(parseCollapsedSets(saved)[0].groups[0].contentKeys).toEqual([null])
  })

  it("falls back to the first pane for a focus out of range", () => {
    const saved = [{ id: "s", collapsedAt: 1, groups: [{ ...group, activeIndex: 4 }] }]
    expect(parseCollapsedSets(saved)[0].groups[0].activeIndex).toBe(0)
  })
})
