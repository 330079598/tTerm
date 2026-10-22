import { describe, expect, it } from "vitest"

import {
  compareTabIds,
  getSiblingTabId,
  getTabIdAtPosition,
  resolveTabShortcutTarget,
} from "@/lib/tabNavigation"

const TABS = ["t1", "t2", "t3"]

describe("getSiblingTabId", () => {
  it("moves forward and wraps around", () => {
    expect(getSiblingTabId(TABS, "t1", 1)).toBe("t2")
    expect(getSiblingTabId(TABS, "t3", 1)).toBe("t1")
  })

  it("moves backward and wraps around", () => {
    expect(getSiblingTabId(TABS, "t2", -1)).toBe("t1")
    expect(getSiblingTabId(TABS, "t1", -1)).toBe("t3")
  })

  it("stays put when there is a single tab", () => {
    expect(getSiblingTabId(["t1"], "t1", 1)).toBe("t1")
    expect(getSiblingTabId(["t1"], "t1", -1)).toBe("t1")
  })

  it("returns undefined without an active tab or unknown id", () => {
    expect(getSiblingTabId(TABS, null, 1)).toBeUndefined()
    expect(getSiblingTabId(TABS, "missing", 1)).toBeUndefined()
    expect(getSiblingTabId([], "t1", 1)).toBeUndefined()
  })
})

describe("getTabIdAtPosition", () => {
  it("returns 1-based positions", () => {
    expect(getTabIdAtPosition(TABS, 1)).toBe("t1")
    expect(getTabIdAtPosition(TABS, 3)).toBe("t3")
  })

  it("returns undefined for out-of-range positions", () => {
    expect(getTabIdAtPosition(TABS, 0)).toBeUndefined()
    expect(getTabIdAtPosition(TABS, 4)).toBeUndefined()
  })

  it("returns the last tab for position 9", () => {
    expect(getTabIdAtPosition(TABS, 9)).toBe("t3")
    const manyTabs = Array.from({ length: 12 }, (_, index) => `t${index + 1}`)
    expect(getTabIdAtPosition(manyTabs, 9)).toBe("t12")
    expect(getTabIdAtPosition(manyTabs, 8)).toBe("t8")
    expect(getTabIdAtPosition([], 9)).toBeUndefined()
  })
})

describe("resolveTabShortcutTarget", () => {
  it("switches to a tab other than the active one", () => {
    expect(resolveTabShortcutTarget("t2", "t1")).toEqual({ kind: "switch", tabId: "t2" })
  })

  it("reports the active tab as the current target", () => {
    expect(resolveTabShortcutTarget("t1", "t1")).toEqual({ kind: "current", tabId: "t1" })
  })

  it("reports no target when the shortcut found no tab", () => {
    expect(resolveTabShortcutTarget(undefined, "t1")).toEqual({ kind: "none" })
  })
})

describe("compareTabIds", () => {
  it("orders tabs as they were opened, by number rather than by text", () => {
    expect(["tab-10", "tab-2", "tab-1"].sort(compareTabIds)).toEqual(["tab-1", "tab-2", "tab-10"])
  })

  it("falls back to the text for ids that are not numbered", () => {
    expect(["b", "tab-1", "a"].sort(compareTabIds)).toEqual(["a", "b", "tab-1"])
  })
})
