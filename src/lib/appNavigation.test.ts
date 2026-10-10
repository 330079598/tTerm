import { describe, expect, it, vi } from "vitest"

import {
  handleAppPageRequests,
  onLogsTabRequest,
  showLogsForTab,
  takeLogsTabRequest,
} from "@/lib/appNavigation"

describe("appNavigation", () => {
  it("opens the logs page and hands the tab over once", () => {
    const opened = vi.fn()
    const notified = vi.fn()
    const stopHandling = handleAppPageRequests(opened)
    const stopListening = onLogsTabRequest(notified)

    showLogsForTab({ tabId: "tab-1", sessionType: "local" })

    expect(opened).toHaveBeenCalledWith("terminal-logs")
    expect(notified).toHaveBeenCalledTimes(1)
    expect(takeLogsTabRequest()).toEqual({ tabId: "tab-1", sessionType: "local" })
    expect(takeLogsTabRequest()).toBeNull()
    stopListening()
    stopHandling()
  })

  it("stops handling once replaced", () => {
    const first = vi.fn()
    const stopFirst = handleAppPageRequests(first)
    const second = vi.fn()
    const stopSecond = handleAppPageRequests(second)
    stopFirst()
    showLogsForTab({ tabId: "tab-2", sessionType: "local" })
    expect(first).not.toHaveBeenCalled()
    expect(second).toHaveBeenCalledWith("terminal-logs")
    stopSecond()
    takeLogsTabRequest()
  })
})
