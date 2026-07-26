// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react"
import { describe, expect, it } from "vitest"

import { removeTabsFromState, useTabs } from "@/hooks/useTabs"
import type { Tab } from "@/types/tab"

function createTabs(ids: string[], activeTabId: string): Tab[] {
  return ids.map((id) => ({
    id,
    title: id,
    type: "terminal",
    isActive: id === activeTabId,
  }))
}

describe("removeTabsFromState", () => {
  it("keeps the current active tab when it is not removed", () => {
    const result = removeTabsFromState(createTabs(["a", "b", "c", "d"], "d"), "d", ["a", "b"], {
      preferredActiveTabId: "c",
    })

    expect(result.activeTabId).toBe("d")
    expect(result.tabs.map((tab) => tab.id)).toEqual(["c", "d"])
    expect(result.tabs.filter((tab) => tab.isActive).map((tab) => tab.id)).toEqual(["d"])
  })

  it("uses the preferred group tab when the active tab is removed", () => {
    const result = removeTabsFromState(createTabs(["a", "b", "c", "d"], "a"), "a", ["a", "b"], {
      preferredActiveTabId: "c",
    })

    expect(result.activeTabId).toBe("c")
    expect(result.tabs.filter((tab) => tab.isActive).map((tab) => tab.id)).toEqual(["c"])
    expect(result.tabs.find((tab) => tab.id === "c")?.hasConnected).toBe(true)
  })

  it("activates the context tab when closing other tabs in its group", () => {
    const result = removeTabsFromState(createTabs(["a", "b", "c", "d"], "d"), "d", ["a", "c"], {
      preferredActiveTabId: "b",
      activatePreferred: true,
    })

    expect(result.activeTabId).toBe("b")
    expect(result.tabs.map((tab) => tab.id)).toEqual(["b", "d"])
    expect(result.tabs.filter((tab) => tab.isActive).map((tab) => tab.id)).toEqual(["b"])
  })

  it("clears the active tab when all tabs are removed", () => {
    const result = removeTabsFromState(createTabs(["a", "b"], "a"), "a", ["a", "b"])

    expect(result.activeTabId).toBeNull()
    expect(result.tabs).toEqual([])
  })

  it("does not change state when none of the requested tabs exist", () => {
    const tabs = createTabs(["a", "b"], "b")
    const result = removeTabsFromState(tabs, "b", ["missing"])

    expect(result).toEqual({ tabs, activeTabId: "b" })
    expect(result.tabs).toBe(tabs)
  })
})

describe("useTabs atomic state updates", () => {
  it("uses the latest active tab when activation and removal are batched", () => {
    const { result } = renderHook(() => useTabs())

    act(() => {
      result.current.restoreSession(createTabs(["a", "b", "c", "d"], "a"), "a")
    })

    act(() => {
      result.current.setActiveTab("c")
      result.current.removeTabs(["a", "b"], { preferredActiveTabId: "d" })
    })

    expect(result.current.activeTabId).toBe("c")
    expect(result.current.tabs.map((tab) => tab.id)).toEqual(["c", "d"])
    expect(result.current.tabs.filter((tab) => tab.isActive).map((tab) => tab.id)).toEqual(["c"])

    act(() => {
      result.current.updateTab("d", (tab) => ({ ...tab, isActive: true }))
    })

    expect(result.current.activeTabId).toBe("c")
    expect(result.current.tabs.filter((tab) => tab.isActive).map((tab) => tab.id)).toEqual(["c"])
  })
})
