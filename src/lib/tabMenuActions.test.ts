import { describe, expect, it } from "vitest"

import { getTabPinMenuAction, getTabSplitMenuActions } from "@/lib/tabMenuActions"

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

describe("getTabPinMenuAction", () => {
  it("pins or unpins an SSH tab's connection header", () => {
    const actions = (tab: Parameters<typeof getTabPinMenuAction>[0]) =>
      getTabPinMenuAction(tab, t).map((item) => item.action)
    expect(actions({ type: "ssh", connectionHeaderPinned: false })).toEqual(["pin-header"])
    expect(actions({ type: "ssh" })).toEqual(["unpin-header"])
    expect(actions({ type: "terminal" })).toEqual([])
  })
})
