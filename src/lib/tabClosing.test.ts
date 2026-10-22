import { describe, expect, it } from "vitest"

import {
  getTabCloseContext,
  getTabCloseMenuActions,
  getTabIdsForCloseAction,
} from "@/lib/tabClosing"

describe("tab closing context", () => {
  it("calculates close targets from the supplied group order", () => {
    const groupTabIds = ["group-b", "group-d", "group-a"]

    expect(getTabIdsForCloseAction(groupTabIds, "group-d", "close-left")).toEqual(["group-b"])
    expect(getTabIdsForCloseAction(groupTabIds, "group-d", "close-right")).toEqual(["group-a"])
    expect(getTabIdsForCloseAction(groupTabIds, "group-d", "close-others")).toEqual([
      "group-b",
      "group-a",
    ])
  })

  it("does not include tabs from another group", () => {
    const groupTabIds = ["a", "c"]

    expect(getTabIdsForCloseAction(groupTabIds, "a", "close-right")).toEqual(["c"])
    expect(getTabIdsForCloseAction(groupTabIds, "a", "close-others")).not.toContain("b")
  })

  it("reports empty sides so invalid menu actions can be disabled", () => {
    const first = getTabCloseContext(["a", "b"], "a")
    const last = getTabCloseContext(["a", "b"], "b")
    const only = getTabCloseContext(["a"], "a")

    expect(first?.leftIds).toEqual([])
    expect(last?.rightIds).toEqual([])
    expect(only?.otherIds).toEqual([])
  })

  it("builds consistent menu actions from the group context", () => {
    const actions = getTabCloseMenuActions(["a", "b"], "a", {
      closeTab: "Close",
      closeOtherTabs: "Close others",
      closeTabsToLeft: "Close left",
      closeTabsToRight: "Close right",
    })

    expect(actions.map((action) => action.action)).toEqual([
      "close-tab",
      "close-others",
      "close-left",
      "close-right",
    ])
    expect(actions.find((action) => action.action === "close-left")?.disabled).toBe(true)
    expect(actions.find((action) => action.action === "close-right")?.disabled).toBe(false)
  })

  it("shows the close shortcut only for a tab with one pane, which it closes", () => {
    const labels = {
      closeTab: "Close",
      closeOtherTabs: "Close others",
      closeTabsToLeft: "Close left",
      closeTabsToRight: "Close right",
    }
    const shortcut = (isSplit: boolean) =>
      getTabCloseMenuActions(["a"], "a", labels, isSplit).find(
        (action) => action.action === "close-tab"
      )?.shortcut

    expect(shortcut(false)).toBe("tabs.closeActive")
    expect(shortcut(true)).toBeUndefined()
  })
})
