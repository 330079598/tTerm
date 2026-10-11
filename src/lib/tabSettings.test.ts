import { describe, expect, it } from "vitest"

import { DEFAULT_TAB_SETTINGS, normalizeTabSettings } from "@/lib/tabSettings"

describe("normalizeTabSettings", () => {
  it("knows only the two width modes", () => {
    expect(normalizeTabSettings({ tab_width_mode: "standard" }).tab_width_mode).toBe("standard")
    expect(normalizeTabSettings({ tab_width_mode: "wide" }).tab_width_mode).toBe("adaptive")
  })

  it("keeps the standard width in range", () => {
    const width = (value: unknown) =>
      normalizeTabSettings({ tab_standard_width: value }).tab_standard_width
    expect(width(150.4)).toBe(150)
    expect(width(20)).toBe(80)
    expect(width(999)).toBe(300)
  })

  it("fills in the defaults", () => {
    expect(normalizeTabSettings({})).toEqual(DEFAULT_TAB_SETTINGS)
  })
})
