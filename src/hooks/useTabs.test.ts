// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react"
import { describe, expect, it } from "vitest"

import { useTabs } from "@/hooks/useTabs"
import { BUILTIN_PANE_TEMPLATES } from "@/lib/paneTemplates"
import { getPaneIds } from "@/lib/tabGroups"
import { getDuplicateTabTitle, removeTabsFromState } from "@/lib/tabsState"
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

describe("useTabs automatic titles", () => {
  const titles = (result: { current: ReturnType<typeof useTabs> }) =>
    result.current.tabs.map((tab) => tab.title)

  const localTab = (title: string): Omit<Tab, "id" | "isActive"> => ({
    title,
    autoTitle: true,
    type: "terminal",
    connection: { type: "terminal", cwd: "/work/tTerm" },
  })

  it("leaves repeated automatic titles and their copies unnumbered", () => {
    const { result } = renderHook(() => useTabs())
    let second = ""
    act(() => {
      result.current.addTab(localTab("tTerm"))
      second = result.current.addTab(localTab("tTerm"))
    })
    act(() => {
      result.current.duplicateTab(second)
    })

    expect(titles(result)).toEqual(["tTerm", "tTerm", "tTerm"])
    expect(result.current.tabs[2]).toMatchObject({ autoTitle: true, duplicateBaseTitle: undefined })
  })

  it("sets apart tabs in different directories of the same name", () => {
    const { result } = renderHook(() => useTabs())
    let second = ""
    act(() => {
      result.current.addTab(localTab("tTerm"))
      second = result.current.addTab(localTab("tTerm"))
    })
    act(() => {
      result.current.updateTab(second, (tab) => ({
        ...tab,
        connection: { type: "terminal", cwd: "/backup/tTerm" },
      }))
    })

    expect(titles(result)).toEqual(["work/tTerm", "backup/tTerm"])
  })

  it("stops following the shell once the user renames the tab", () => {
    const { result } = renderHook(() => useTabs())
    let id = ""
    act(() => {
      id = result.current.addTab(localTab("tTerm"))
    })
    act(() => {
      result.current.renameTab(id, "builds")
    })
    act(() => {
      result.current.duplicateTab(id)
    })

    expect(titles(result)).toEqual(["builds", "builds-2"])
    expect(result.current.tabs.every((tab) => !tab.autoTitle)).toBe(true)
  })
})

describe("useTabs split tabs", () => {
  const panes = (result: { current: ReturnType<typeof useTabs> }) =>
    result.current.groups.map((group) => getPaneIds(group.root))

  function setup(ids: string[], layout?: unknown) {
    const hook = renderHook(() => useTabs())
    act(() => {
      hook.result.current.restoreSession(createTabs(ids, ids[0]), ids[0], layout)
    })
    return hook
  }

  it("gives every tab a tab of the bar of its own at first", () => {
    const { result } = setup(["a", "b"])
    expect(panes(result)).toEqual([["a"], ["b"]])
  })

  it("opens a split's copy as a pane beside its source, and lists the tabs in bar order", () => {
    const { result } = setup(["a", "b"])
    let copy: string | null = null
    act(() => {
      copy = result.current.duplicateTab("a", "right")
    })
    expect(panes(result)).toEqual([["a", copy], ["b"]])
    expect(result.current.tabs.map((tab) => tab.id)).toEqual(["a", copy, "b"])
    expect(result.current.activeTabId).toBe(copy)
  })

  it("moves tabs of the bar with their panes", () => {
    const { result } = setup(["a", "b", "c"])
    act(() => {
      result.current.mergeGroup("c", "a", "below")
    })
    act(() => {
      result.current.moveTab(0, 1)
    })
    expect(panes(result)).toEqual([["b"], ["a", "c"]])
    expect(result.current.tabs.map((tab) => tab.id)).toEqual(["b", "a", "c"])
  })

  it("focuses the next pane of a tab when its active pane closes", () => {
    const { result } = setup(["a", "b", "c"])
    act(() => {
      result.current.mergeGroup("b", "a", "right")
    })
    act(() => {
      result.current.setActiveTab("a")
    })
    act(() => {
      result.current.removeTabs(["a"])
    })
    expect(result.current.activeTabId).toBe("b")
    expect(panes(result)).toEqual([["b"], ["c"]])
  })

  it("restores the saved split for the tabs that came back", () => {
    const layout = {
      version: 2,
      groups: [
        {
          root: {
            type: "split",
            orientation: "vertical",
            sizes: [0.3, 0.7],
            children: [
              { type: "pane", tabId: "b" },
              { type: "pane", tabId: "a" },
            ],
          },
          activeTabId: "a",
        },
      ],
    }
    const { result } = setup(["a", "b", "c"], layout)
    expect(panes(result)).toEqual([["b", "a"], ["c"]])
    expect(result.current.tabs.map((tab) => tab.id)).toEqual(["b", "a", "c"])
    expect(new Set(result.current.groups.map((group) => group.id)).size).toBe(2)
  })

  it("breaks a pane out into a new tab of the bar and focuses it", () => {
    const { result } = setup(["a", "b"])
    act(() => {
      result.current.mergeGroup("b", "a", "right")
    })
    act(() => {
      result.current.breakOutPane("a", 0)
    })
    expect(panes(result)).toEqual([["a"], ["b"]])
    expect(result.current.activeTabId).toBe("a")
  })
})

describe("useTabs layout templates", () => {
  const panes = (result: { current: ReturnType<typeof useTabs> }) =>
    result.current.groups.map((group) => getPaneIds(group.root))

  function setup(ids: string[]) {
    const hook = renderHook(() => useTabs())
    act(() => {
      hook.result.current.restoreSession(createTabs(ids, ids[0]), ids[0])
    })
    return hook
  }

  it("fills the panes a layout adds with copies of the pane it was applied to", () => {
    const { result } = setup(["a", "b"])
    act(() => {
      result.current.applyLayoutTemplate("a", BUILTIN_PANE_TEMPLATES[2].root)
    })
    const [first] = panes(result)
    expect(first).toHaveLength(3)
    expect(first[0]).toBe("a")
    expect(panes(result)[1]).toEqual(["b"])
    expect(result.current.tabs.map((tab) => tab.title)).toEqual(["a", "a-2", "a-3", "b"])
    expect(result.current.activeTabId).toBe("a")
  })

  it("moves the panes a smaller layout has no room for into tabs of their own", () => {
    const { result } = setup(["a", "b", "c"])
    act(() => {
      result.current.mergeGroup("b", "a", "right")
      result.current.mergeGroup("c", "b", "right")
    })
    act(() => {
      result.current.applyLayoutTemplate("c", BUILTIN_PANE_TEMPLATES[1].root)
    })
    expect(panes(result)).toEqual([["a", "c"], ["b"]])
    expect(result.current.groups[0].root).toMatchObject({ orientation: "vertical" })
    expect(result.current.activeTabId).toBe("c")
  })

  it("leaves a page tab as it is when the layout needs copies of it", () => {
    const { result } = renderHook(() => useTabs())
    act(() => {
      result.current.openSettingsTab("Settings")
    })
    const before = result.current.groups
    act(() => {
      result.current.applyLayoutTemplate(result.current.tabs[0].id, BUILTIN_PANE_TEMPLATES[0].root)
    })
    expect(result.current.groups).toBe(before)
    expect(result.current.tabs).toHaveLength(1)
  })
})
