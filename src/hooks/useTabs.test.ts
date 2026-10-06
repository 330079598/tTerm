// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react"
import { describe, expect, it } from "vitest"

import { getDuplicateTabTitle, removeTabsFromState, useTabs } from "@/hooks/useTabs"
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

describe("getDuplicateTabTitle", () => {
  it("appends the first free index", () => {
    expect(getDuplicateTabTitle("web", ["web"])).toBe("web-2")
    expect(getDuplicateTabTitle("web", ["web", "web-2", "web-4"])).toBe("web-3")
  })
})

describe("useTabs duplicateTab", () => {
  function setup(ids: string[]) {
    const hook = renderHook(() => useTabs())
    act(() => {
      hook.result.current.restoreSession(createTabs(ids, ids[0]), ids[0])
    })
    return hook
  }

  function duplicate(result: { current: ReturnType<typeof useTabs> }, title: string) {
    const tab = result.current.tabs.find((candidate) => candidate.title === title)
    let newId: string | null = null
    act(() => {
      newId = result.current.duplicateTab(tab!.id)
    })
    return newId!
  }

  const titles = (result: { current: ReturnType<typeof useTabs> }) =>
    result.current.tabs.map((tab) => tab.title)

  it("names duplicates with incrementing indexes", () => {
    const { result } = setup(["web"])
    duplicate(result, "web")
    duplicate(result, "web")

    expect(titles(result)).toEqual(["web", "web-2", "web-3"])
  })

  it("numbers a copy of a copy from the original name, even after it is closed", () => {
    const { result } = setup(["web"])
    duplicate(result, "web")
    act(() => {
      result.current.removeTabs(["web"])
    })
    duplicate(result, "web-2")

    expect(titles(result)).toEqual(["web-2", "web-3"])
  })

  it("treats a numeric suffix in the original name as part of the name", () => {
    const { result } = setup(["web", "web-2"])
    duplicate(result, "web-2")

    expect(titles(result)).toEqual(["web", "web-2", "web-2-2"])
  })

  it("numbers from the new name once a copy is renamed", () => {
    const { result } = setup(["web"])
    const copyId = duplicate(result, "web")
    act(() => {
      result.current.renameTab(copyId, "db")
    })
    duplicate(result, "db")

    expect(titles(result)).toEqual(["web", "db", "db-2"])
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

describe("useTabs generated titles", () => {
  const titles = (result: { current: ReturnType<typeof useTabs> }) =>
    result.current.tabs.map((tab) => tab.title)

  const shellTab = (title: string): Omit<Tab, "id" | "isActive"> => ({
    title,
    numberTitle: true,
    type: "terminal",
  })

  it("numbers a repeated shell name, and copies from it", () => {
    const { result } = renderHook(() => useTabs())
    let second = ""
    act(() => {
      result.current.addTab(shellTab("pwsh"))
      second = result.current.addTab(shellTab("pwsh"))
      result.current.addTab(shellTab("cmd"))
    })
    act(() => {
      result.current.duplicateTab(second)
    })

    expect(titles(result)).toEqual(["pwsh", "pwsh-2", "cmd", "pwsh-3"])
    expect(result.current.tabs.every((tab) => tab.numberTitle === undefined)).toBe(true)
  })

  it("numbers restored tabs around the names kept as saved", () => {
    const { result } = renderHook(() => useTabs())
    act(() => {
      result.current.restoreSession(
        [
          { id: "tab-1", type: "terminal", isActive: true, title: "cmd", numberTitle: true },
          { id: "tab-2", type: "terminal", isActive: false, title: "cmd-2" },
          { id: "tab-3", type: "terminal", isActive: false, title: "cmd", numberTitle: true },
        ],
        "tab-1"
      )
    })

    expect(titles(result)).toEqual(["cmd", "cmd-2", "cmd-3"])
  })
})
