import { describe, expect, it } from "vitest"

import {
  applyTemplateToGroups,
  BUILTIN_PANE_TEMPLATES,
  countTemplatePanes,
  fillShape,
  MAX_TEMPLATE_PANES,
  normalizeLayoutShape,
  normalizePaneTemplates,
  shapeOfTree,
  type LayoutShape,
} from "@/lib/paneTemplates"
import { getPaneIds, paneLeaf, type PaneNode, type TabGroup } from "@/lib/tabGroups"

const pane: LayoutShape = { type: "pane" }
const row = (children: LayoutShape[], sizes = children.map(() => 1 / children.length)) => ({
  type: "split" as const,
  orientation: "horizontal" as const,
  children,
  sizes,
})

const group = (id: string, root: PaneNode, activeTabId = getPaneIds(root)[0]): TabGroup => ({
  id,
  root,
  activeTabId,
})

describe("shapes", () => {
  it("drops a tree's panes and puts others back in reading order", () => {
    const tree: PaneNode = {
      type: "split",
      orientation: "horizontal",
      children: [
        paneLeaf("a"),
        {
          type: "split",
          orientation: "vertical",
          children: [paneLeaf("b"), paneLeaf("c")],
          sizes: [0.5, 0.5],
        },
      ],
      sizes: [0.6, 0.4],
    }
    const shape = shapeOfTree(tree)
    expect(countTemplatePanes(shape)).toBe(3)
    expect(getPaneIds(fillShape(shape, ["x", "y", "z"]))).toEqual(["x", "y", "z"])
    expect(fillShape(shape, ["x", "y", "z"])).toMatchObject({ sizes: [0.6, 0.4] })
  })

  it("gives every built-in template a distinct id and at least two panes", () => {
    const ids = new Set(BUILTIN_PANE_TEMPLATES.map((template) => template.id))
    expect(ids.size).toBe(BUILTIN_PANE_TEMPLATES.length)
    for (const template of BUILTIN_PANE_TEMPLATES) {
      expect(normalizeLayoutShape(template.root)).toEqual(template.root)
    }
  })
})

describe("normalizeLayoutShape", () => {
  it("scales sizes to one and fills in missing ones", () => {
    expect(normalizeLayoutShape(row([pane, pane], [1, 3]))).toEqual(row([pane, pane], [0.25, 0.75]))
    expect(normalizeLayoutShape({ ...row([pane, pane]), sizes: "x" })).toEqual(row([pane, pane]))
  })

  it("rejects a lone pane, bad nodes and too many panes", () => {
    expect(normalizeLayoutShape(pane)).toBeNull()
    expect(
      normalizeLayoutShape({ type: "split", orientation: "diagonal", children: [pane, pane] })
    ).toBeNull()
    expect(normalizeLayoutShape(row([pane]))).toBeNull()
    expect(normalizeLayoutShape(row([pane, { type: "box" } as never]))).toBeNull()
    const tooMany = row(Array.from({ length: MAX_TEMPLATE_PANES + 1 }, () => pane))
    expect(normalizeLayoutShape(tooMany)).toBeNull()
  })
})

describe("normalizePaneTemplates", () => {
  it("keeps valid templates and drops broken ones, repeats and built-in ids", () => {
    const templates = normalizePaneTemplates([
      { id: "a", name: " Ops ", root: row([pane, pane]) },
      { id: "a", name: "again", root: row([pane, pane]) },
      { id: "builtin:columns-2", name: "fake", root: row([pane, pane]) },
      { id: "b", name: "broken", root: pane },
      "nonsense",
    ])
    expect(templates).toEqual([{ id: "a", name: "Ops", root: row([pane, pane]) }])
    expect(normalizePaneTemplates(undefined)).toEqual([])
  })
})

describe("applyTemplateToGroups", () => {
  let next = 0
  const createId = () => `new-${++next}`

  it("fills the shape with the panes there are, then the new ones", () => {
    const groups = [group("g1", paneLeaf("a")), group("g2", paneLeaf("b"))]
    const result = applyTemplateToGroups(groups, "a", row([pane, pane, pane]), ["c", "d"], createId)
    expect(result.map((candidate) => getPaneIds(candidate.root))).toEqual([["a", "c", "d"], ["b"]])
    expect(result[0]).toMatchObject({ id: "g1", activeTabId: "a" })
  })

  it("keeps the active pane when the shape is smaller, and moves the rest out after it", () => {
    const groups = [
      group(
        "g1",
        {
          type: "split",
          orientation: "vertical",
          children: [paneLeaf("a"), paneLeaf("b"), paneLeaf("c")],
          sizes: [1 / 3, 1 / 3, 1 / 3],
        },
        "c"
      ),
      group("g2", paneLeaf("d")),
    ]
    const result = applyTemplateToGroups(groups, "c", row([pane, pane]), [], createId)
    expect(result.map((candidate) => getPaneIds(candidate.root))).toEqual([
      ["a", "c"],
      ["b"],
      ["d"],
    ])
    expect(result[0].root).toMatchObject({ orientation: "horizontal" })
  })

  it("changes nothing without enough panes to fill the shape", () => {
    const groups = [group("g1", paneLeaf("a"))]
    expect(applyTemplateToGroups(groups, "a", row([pane, pane]), [], createId)).toBe(groups)
  })
})
