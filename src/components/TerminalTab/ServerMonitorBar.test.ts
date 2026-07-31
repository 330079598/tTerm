import { describe, expect, it } from "vitest"

import {
  calculateNetworkRate,
  formatNetworkRate,
  selectMostUsedDisk,
  type DiskMetrics,
} from "@/components/TerminalTab/ServerMonitorBar"

const network = {
  interface: "eth0",
  receivedBytes: 1000,
  transmittedBytes: 2000,
  receiveErrors: 0,
  transmitErrors: 0,
  receiveDropped: 0,
  transmitDropped: 0,
}

describe("server monitor network rates", () => {
  it("calculates byte rates from cumulative counters", () => {
    const rate = calculateNetworkRate(
      { metrics: network, capturedAt: 1000 },
      { ...network, receivedBytes: 5000, transmittedBytes: 4000 },
      3000
    )

    expect(rate).toEqual({
      receivedBytesPerSecond: 2000,
      transmittedBytesPerSecond: 1000,
    })
  })

  it("ignores interface changes and reset counters", () => {
    expect(
      calculateNetworkRate(
        { metrics: network, capturedAt: 1000 },
        { ...network, interface: "wlan0" },
        2000
      )
    ).toBeUndefined()
    expect(
      calculateNetworkRate(
        { metrics: network, capturedAt: 1000 },
        { ...network, receivedBytes: 1 },
        2000
      )
    ).toBeUndefined()
  })

  it("formats compact status bar values", () => {
    expect(formatNetworkRate(undefined)).toBe("--")
    expect(formatNetworkRate(2048)).toBe("2.00K/s")
    expect(formatNetworkRate(2 * 1024 * 1024)).toBe("2.00M/s")
  })
})

describe("server monitor status bar disk", () => {
  const disk = (mount: string, usedPercent: number): DiskMetrics => ({
    filesystem: `/dev/${mount === "/" ? "root" : mount.slice(1)}`,
    filesystemType: "xfs",
    mount,
    totalKib: 100,
    usedKib: 40,
    availableKib: 60,
    usedPercent,
  })

  it("selects the filesystem with the highest usage", () => {
    expect(selectMostUsedDisk([disk("/", 47), disk("/boot", 15), disk("/data", 82)])?.mount).toBe(
      "/data"
    )
  })

  it("keeps the first filesystem when usage is tied", () => {
    expect(selectMostUsedDisk([disk("/", 47), disk("/data", 47)])?.mount).toBe("/")
    expect(selectMostUsedDisk([])).toBeUndefined()
  })
})
