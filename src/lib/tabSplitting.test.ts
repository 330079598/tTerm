import { describe, expect, it } from "vitest"

import { getTabSplitMenuActions } from "@/lib/tabSplitting"

const t = ((key: string) => key) as Parameters<typeof getTabSplitMenuActions>[1]

describe("getTabSplitMenuActions", () => {
  it("offers the four splits for terminal tabs only", () => {
    expect(getTabSplitMenuActions({ type: "ssh" }, t).map((item) => item.action)).toEqual([
      "split-right",
      "split-left",
      "split-down",
      "split-above",
    ])
    expect(getTabSplitMenuActions({ type: "terminal" }, t)).toHaveLength(4)
    expect(getTabSplitMenuActions({ type: "settings" }, t)).toEqual([])
  })
})
