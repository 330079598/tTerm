import { describe, expect, it } from "vitest"

import {
  DEFAULT_TAB_AGENT_INDICATOR,
  normalizeTabAgentIndicator,
  tabAgentColorVariables,
} from "@/lib/tabAgentIndicator"

describe("normalizeTabAgentIndicator", () => {
  it("fills in the defaults", () => {
    expect(normalizeTabAgentIndicator(undefined)).toEqual(DEFAULT_TAB_AGENT_INDICATOR)
    expect(normalizeTabAgentIndicator("dots")).toEqual(DEFAULT_TAB_AGENT_INDICATOR)
  })

  it("keeps known styles and drops unknown ones", () => {
    const indicator = normalizeTabAgentIndicator({ working: "braille", waiting: "spin" })
    expect(indicator.working).toBe("braille")
    expect(indicator.waiting).toBe("bars")
  })

  it("keeps only #rrggbb colors, lowercased", () => {
    const indicator = normalizeTabAgentIndicator({
      working_color: "#3B9AE1",
      waiting_color: "red",
    })
    expect(indicator.working_color).toBe("#3b9ae1")
    expect(indicator.waiting_color).toBe("")
  })
})

describe("tabAgentColorVariables", () => {
  it("leaves the theme's color when none is chosen", () => {
    expect(tabAgentColorVariables("")).toBeUndefined()
  })

  it("sets the color and its faint shade", () => {
    expect(tabAgentColorVariables("#3b9ae1")).toEqual({
      "--tab-agent-color": "#3b9ae1",
      "--tab-agent-faint": "#3b9ae138",
    })
  })
})
