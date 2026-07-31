import { describe, expect, it } from "vitest"

import { calculateNetworkRate, formatNetworkRate } from "@/components/TerminalTab/ServerMonitorBar"

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
