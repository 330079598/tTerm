// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { CAPS_LOCK_NOTICE_MS, useCapsLockNotice } from "@/components/TerminalTab/useCapsLockNotice"

describe("useCapsLockNotice", () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("stays quiet while Caps Lock is off", () => {
    const { result } = renderHook(() => useCapsLockNotice(false))
    expect(result.current).toBe(false)
  })

  it("shows for a moment when Caps Lock turns on", () => {
    const { result, rerender } = renderHook(({ on }) => useCapsLockNotice(on), {
      initialProps: { on: false },
    })
    rerender({ on: true })
    expect(result.current).toBe(true)

    act(() => {
      vi.advanceTimersByTime(CAPS_LOCK_NOTICE_MS)
    })
    expect(result.current).toBe(false)
  })

  it("shows when Caps Lock is already on as the prompt appears", () => {
    const { result } = renderHook(() => useCapsLockNotice(true))
    expect(result.current).toBe(true)
  })

  it("shows again each time Caps Lock turns back on", () => {
    const { result, rerender } = renderHook(({ on }) => useCapsLockNotice(on), {
      initialProps: { on: true },
    })
    act(() => {
      vi.advanceTimersByTime(CAPS_LOCK_NOTICE_MS)
    })
    rerender({ on: false })
    rerender({ on: true })
    expect(result.current).toBe(true)
  })

  it("hides when Caps Lock turns off", () => {
    const { result, rerender } = renderHook(({ on }) => useCapsLockNotice(on), {
      initialProps: { on: true },
    })
    rerender({ on: false })
    expect(result.current).toBe(false)
  })
})
