import { describe, expect, it } from "vitest"

import {
  buildGroupTree,
  groupAndDescendantIds,
  normalizeGroupPath,
  visibleGroupNodes,
  visibleTreeProfiles,
  type GroupNode,
} from "@/lib/profileGroups"
import type { ProfileGroup, SavedProfile } from "@/types/tab"

const group = (id: string, name: string, parentId: string | null = null): ProfileGroup => ({
  id,
  name,
  parentId,
})

const profile = (id: string, groupId?: string): SavedProfile => ({
  id,
  name: id,
  group_id: groupId,
  connection_type: "ssh",
  remember_password: false,
  keepalive_interval_secs: 15,
  keepalive_count_max: 3,
})

const outline = (nodes: GroupNode[]): unknown[] =>
  nodes.map((node) => [
    node.path,
    node.total,
    node.profiles.map((item) => item.id),
    outline(node.children),
  ])

const UNGROUPED = "__ungrouped__"

describe("normalizeGroupPath", () => {
  it("trims the names and drops empty ones", () => {
    expect(normalizeGroupPath(" prod /web// ")).toBe("prod / web")
    expect(normalizeGroupPath(" / ")).toBe("")
  })
})

describe("buildGroupTree", () => {
  it("nests groups by name and counts the profiles inside each", () => {
    const tree = buildGroupTree(
      [group("w", "web", "p"), group("p", "prod"), group("a", "api", "p"), group("d", "dev")],
      [profile("1", "w"), profile("2", "p"), profile("3"), profile("4", "gone"), profile("5", "w")]
    )
    expect(outline(tree.roots)).toEqual([
      ["dev", 0, [], []],
      [
        "prod",
        3,
        ["2"],
        [
          ["prod / api", 0, [], []],
          ["prod / web", 2, ["1", "5"], []],
        ],
      ],
    ])
    expect(tree.ungrouped.map((item) => item.id)).toEqual(["3", "4"])
    expect(tree.nodes.get("w")?.depth).toBe(1)
  })

  it("puts a group whose parent is gone, or one in a loop, at the top", () => {
    const tree = buildGroupTree(
      [group("o", "orphan", "gone"), group("x", "x", "y"), group("y", "y", "x")],
      []
    )
    expect(outline(tree.roots)).toEqual([
      ["orphan", 0, [], []],
      ["x", 0, [], [["x / y", 0, [], []]]],
    ])
  })
})

describe("tree walks", () => {
  const tree = buildGroupTree(
    [group("p", "prod"), group("w", "web", "p"), group("d", "dev")],
    [profile("1", "w"), profile("2", "p"), profile("3"), profile("4", "d")]
  )

  it("lists a group's own id and those inside it", () => {
    expect([...groupAndDescendantIds(tree, "p")].sort()).toEqual(["p", "w"])
    expect(groupAndDescendantIds(tree, "missing").size).toBe(0)
  })

  it("lists the groups in sight, skipping those in a collapsed one", () => {
    const ids = (collapsed: string[]) =>
      visibleGroupNodes(tree, new Set(collapsed)).map((node) => node.group.id)
    expect(ids([])).toEqual(["d", "p", "w"])
    expect(ids(["p"])).toEqual(["d", "p"])
  })

  it("lists the profiles in sight in the order the tree shows them", () => {
    const ids = (collapsed: string[]) =>
      visibleTreeProfiles(tree, new Set(collapsed), UNGROUPED).map((item) => item.id)
    expect(ids([])).toEqual(["4", "1", "2", "3"])
    expect(ids(["p", UNGROUPED])).toEqual(["4"])
    expect(ids(["w"])).toEqual(["4", "2", "3"])
  })
})
