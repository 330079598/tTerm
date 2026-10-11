import { describe, expect, it } from "vitest"

import { placeOverflowPanel } from "@/components/TabBar/overflowPanelStyle"

describe("placeOverflowPanel", () => {
  it("opens under its trigger, right-aligned to it", () => {
    expect(placeOverflowPanel({ right: 900, bottom: 36 }, 1200, 800)).toEqual({
      left: 580,
      top: 40,
      width: 320,
      maxHeight: 420,
      "--tab-overflow-panel-max-height": "420px",
    })
  })

  it("stays inside a narrow, short window", () => {
    const style = placeOverflowPanel({ right: 50, bottom: 36 }, 300, 200)
    expect(style.left).toBe(8)
    expect(style.width).toBe(210)
    expect(style.maxHeight).toBe(152)
  })

  it("keeps a usable height when the window is shorter than that", () => {
    expect(placeOverflowPanel({ right: 900, bottom: 36 }, 1200, 100).maxHeight).toBe(120)
  })
})
