import { describe, expect, it } from "vitest"

import { getSiblingTabId, getTabIdAtPosition } from "@/lib/tabNavigation"

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
})
