import { describe, expect, it } from "vitest"

import {
  BUILTIN_HIGHLIGHT_SET_ID,
  builtinHighlightSet,
  defaultKeywordHighlightConfig,
  type HighlightSet,
  MAX_HIGHLIGHT_SETS,
} from "@/lib/keywordHighlight/config"
import {
  addHighlightRule,
  addHighlightSet,
  deleteHighlightSet,
  moveHighlightRule,
  newHighlightRule,
  removeHighlightRule,
  restoreBuiltinRules,
  updateHighlightRule,
  updateHighlightSet,
} from "@/lib/keywordHighlight/edit"

function setWithRules(...patterns: string[]): HighlightSet {
  return {
    id: "set",
    name: "Set",
    rules: patterns.map((pattern) => ({ ...newHighlightRule(), id: pattern, pattern })),
  }
}

describe("highlight set edits", () => {
  it("saves the built-in set once it is edited", () => {
    const config = updateHighlightSet(
      defaultKeywordHighlightConfig(),
      BUILTIN_HIGHLIGHT_SET_ID,
      (set) => ({ ...set, rules: [] })
    )
    expect(config.sets).toEqual([{ id: BUILTIN_HIGHLIGHT_SET_ID, name: "", rules: [] }])
  })

  it("adds a copy of a set with rules of its own", () => {
    const builtin = builtinHighlightSet()
    const added = addHighlightSet(defaultKeywordHighlightConfig(), "Copy", builtin)
    const copy = added?.config.sets?.find((set) => set.id === added.id)
    expect(copy?.name).toBe("Copy")
    expect(copy?.rules.map((rule) => rule.pattern)).toEqual(
      builtin.rules.map((rule) => rule.pattern)
    )
    expect(copy?.rules[0].id).not.toBe(builtin.rules[0].id)
  })

  it("stops adding sets at the limit", () => {
    const sets = Array.from({ length: MAX_HIGHLIGHT_SETS }, (_, index) => ({
      id: `s${index}`,
      name: "",
      rules: [],
    }))
    expect(addHighlightSet({ ...defaultKeywordHighlightConfig(), sets }, "One more")).toBeNull()
  })

  it("moves the default to the first set left and never deletes the last one", () => {
    const config = {
      ...defaultKeywordHighlightConfig(),
      default_set: "b",
      sets: [setWithRules(), { id: "b", name: "B", rules: [] }].map((set, index) =>
        index === 0 ? { ...set, id: "a" } : set
      ),
    }
    const deleted = deleteHighlightSet(config, "b")
    expect(deleted.default_set).toBe("a")
    expect(deleted.sets?.map((set) => set.id)).toEqual(["a"])
    expect(deleteHighlightSet(deleted, "a")).toBe(deleted)
  })

  it("restores the built-in rules only in the built-in set", () => {
    const emptied = { ...builtinHighlightSet(), rules: [] }
    expect(restoreBuiltinRules(emptied).rules).toEqual(builtinHighlightSet().rules)
    const other = setWithRules("x")
    expect(restoreBuiltinRules(other)).toBe(other)
  })
})

describe("highlight rule edits", () => {
  it("adds, changes and removes rules", () => {
    let set = addHighlightRule(setWithRules("a"), { ...newHighlightRule(), id: "b" })
    set = updateHighlightRule(set, "b", { pattern: "down", foreground: "#ff0000" })
    expect(set.rules[1]).toMatchObject({ pattern: "down", foreground: "#ff0000" })
    expect(removeHighlightRule(set, "a").rules.map((rule) => rule.id)).toEqual(["b"])
  })

  it("moves rules within the set's bounds", () => {
    const set = setWithRules("a", "b", "c")
    const ids = (moved: HighlightSet) => moved.rules.map((rule) => rule.id)
    expect(ids(moveHighlightRule(set, "c", -1))).toEqual(["a", "c", "b"])
    expect(ids(moveHighlightRule(set, "a", 1))).toEqual(["b", "a", "c"])
    expect(moveHighlightRule(set, "a", -1)).toBe(set)
    expect(moveHighlightRule(set, "c", 1)).toBe(set)
  })
})
