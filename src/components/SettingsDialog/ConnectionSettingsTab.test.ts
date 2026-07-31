import { describe, expect, it } from "vitest"

import { reorderMonitorMetrics } from "@/components/SettingsDialog/ConnectionSettingsTab"

describe("status bar metric ordering", () => {
  const metrics = ["cpu", "memory", "network", "disk"] as const

  it("moves a metric forward", () => {
    expect(reorderMonitorMetrics([...metrics], "cpu", "network")).toEqual([
      "memory",
      "network",
      "cpu",
      "disk",
    ])
  })

  it("moves a metric backward", () => {
    expect(reorderMonitorMetrics([...metrics], "disk", "memory")).toEqual([
      "cpu",
      "disk",
      "memory",
      "network",
    ])
  })

  it("keeps the same array when the drop does not change the order", () => {
    const current = [...metrics]

    expect(reorderMonitorMetrics(current, "memory", "memory")).toBe(current)
    expect(reorderMonitorMetrics(current, "memory", "latency")).toBe(current)
  })
})
