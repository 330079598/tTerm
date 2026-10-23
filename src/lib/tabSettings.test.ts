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

  it("shows the hover tooltip only when turned on", () => {
    expect(normalizeTabSettings({ tab_hover_tooltip: true }).tab_hover_tooltip).toBe(true)
    expect(normalizeTabSettings({ tab_hover_tooltip: "yes" }).tab_hover_tooltip).toBe(false)
  })

  it("fills in the defaults", () => {
    expect(normalizeTabSettings({})).toEqual(DEFAULT_TAB_SETTINGS)
  })
})

describe("pane settings", () => {
  it("shows headers unless turned off, and keeps the fade in range", () => {
    expect(normalizeTabSettings({ panes: { show_headers: false } }).panes.show_headers).toBe(false)
    expect(normalizeTabSettings({ panes: { show_headers: "no" } }).panes.show_headers).toBe(true)
    const dim = (value: unknown) =>
      normalizeTabSettings({ panes: { dim_inactive_percent: value } }).panes.dim_inactive_percent
    expect(dim(30.4)).toBe(30)
    expect(dim(-5)).toBe(0)
    expect(dim(200)).toBe(60)
    expect(dim("30")).toBe(0)
  })

  it("drops saved layouts that are broken", () => {
    const panes = normalizeTabSettings({
      panes: { templates: [{ id: "a", name: "x", root: { type: "pane" } }] },
    }).panes
    expect(panes.templates).toEqual([])
  })
})
