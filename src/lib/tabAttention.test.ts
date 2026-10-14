// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from "vitest"
import { act, renderHook } from "@testing-library/react"

import {
  clearTabAttention,
  countTabNotification,
  markTabAttention,
  RUNNING_COMMAND_DELAY_MS,
  setTabRunningCommand,
  useRunningCommands,
  watchUnseenNotificationTotal,
} from "@/lib/tabAttention"

describe("watchUnseenNotificationTotal", () => {
  afterEach(() => {
    clearTabAttention("tab-1")
    clearTabAttention("tab-2")
  })

  it("counts notifications across tabs until each tab is seen", () => {
    const totals: number[] = []
    const stop = watchUnseenNotificationTotal((total) => totals.push(total))

    countTabNotification("tab-1")
    countTabNotification("tab-1")
    countTabNotification("tab-2")
    clearTabAttention("tab-1")
    clearTabAttention("tab-2")
    stop()

    expect(totals).toEqual([0, 1, 2, 3, 1, 0])
  })

  it("stays quiet when only a tab's mark changes", () => {
    const listener = vi.fn()
    const stop = watchUnseenNotificationTotal(listener)

    markTabAttention("tab-1", "bell")
    clearTabAttention("tab-1")
    stop()

    expect(listener).toHaveBeenCalledTimes(1)
  })

  it("stops hearing once unsubscribed", () => {
    const listener = vi.fn()
    watchUnseenNotificationTotal(listener)()

    countTabNotification("tab-1")

    expect(listener).toHaveBeenCalledTimes(1)
  })
})

describe("setTabRunningCommand", () => {
  afterEach(() => {
    act(() => setTabRunningCommand("tab-1", null))
    vi.useRealTimers()
  })

  it("shows a command once it has run a moment, and hides it when it ends", () => {
    vi.useFakeTimers()
    const { result } = renderHook(() => useRunningCommands().get("tab-1"))

    act(() => setTabRunningCommand("tab-1", "npm run dev"))
    expect(result.current).toBeUndefined()
    act(() => vi.advanceTimersByTime(RUNNING_COMMAND_DELAY_MS))
    expect(result.current).toBe("npm run dev")

    act(() => setTabRunningCommand("tab-1", null))
    expect(result.current).toBeUndefined()
  })

  it("never shows a command that ends at once", () => {
    vi.useFakeTimers()
    const { result } = renderHook(() => useRunningCommands().get("tab-1"))

    act(() => setTabRunningCommand("tab-1", "cd src"))
    act(() => setTabRunningCommand("tab-1", null))
    act(() => vi.advanceTimersByTime(RUNNING_COMMAND_DELAY_MS * 2))
    expect(result.current).toBeUndefined()
  })
})
