import { describe, expect, it } from "vitest"

import {
  DEFAULT_MONITOR_VISIBLE_METRICS,
  normalizeMonitorVisibleMetrics,
} from "@/lib/monitorMetrics"

describe("normalizeMonitorVisibleMetrics", () => {
  it("keeps known metrics in their order, once each", () => {
    expect(normalizeMonitorVisibleMetrics(["uptime", "cpu", "bogus", "cpu"])).toEqual([
      "uptime",
      "cpu",
    ])
  })

  it("falls back to the defaults", () => {
    expect(normalizeMonitorVisibleMetrics(undefined)).toEqual(DEFAULT_MONITOR_VISIBLE_METRICS)
    expect(normalizeMonitorVisibleMetrics(["bogus"])).toEqual(DEFAULT_MONITOR_VISIBLE_METRICS)
  })
})
