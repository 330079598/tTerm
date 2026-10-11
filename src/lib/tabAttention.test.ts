// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from "vitest"
import { act, renderHook } from "@testing-library/react"

import {
  RUNNING_COMMAND_DELAY_MS,
  setTabRunningCommand,
  useTabRunningCommand,
} from "@/lib/tabAttention"

describe("setTabRunningCommand", () => {
  afterEach(() => {
    act(() => setTabRunningCommand("tab-1", null))
    vi.useRealTimers()
  })

  it("shows a command once it has run a moment, and hides it when it ends", () => {
    vi.useFakeTimers()
    const { result } = renderHook(() => useTabRunningCommand("tab-1"))

    act(() => setTabRunningCommand("tab-1", "npm run dev"))
    expect(result.current).toBeUndefined()
    act(() => vi.advanceTimersByTime(RUNNING_COMMAND_DELAY_MS))
    expect(result.current).toBe("npm run dev")

    act(() => setTabRunningCommand("tab-1", null))
    expect(result.current).toBeUndefined()
  })

  it("never shows a command that ends at once", () => {
    vi.useFakeTimers()
    const { result } = renderHook(() => useTabRunningCommand("tab-1"))

    act(() => setTabRunningCommand("tab-1", "cd src"))
    act(() => setTabRunningCommand("tab-1", null))
    act(() => vi.advanceTimersByTime(RUNNING_COMMAND_DELAY_MS * 2))
    expect(result.current).toBeUndefined()
  })
})
