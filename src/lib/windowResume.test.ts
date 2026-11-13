// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { subscribeWindowResume } from "@/lib/windowResume"

const cleanups: (() => void)[] = []
let visibility = "visible"

function subscribe(listener = vi.fn()) {
  cleanups.push(subscribeWindowResume(listener))
  return listener
}

function visibilityChanged(state: string) {
  visibility = state
  document.dispatchEvent(new Event("visibilitychange"))
}

describe("window resume detection", () => {
  beforeEach(() => {
    vi.useFakeTimers()
    visibility = "visible"
    vi.spyOn(document, "visibilityState", "get").mockImplementation(
      () => visibility as DocumentVisibilityState
    )
  })

  afterEach(() => {
    cleanups.splice(0).forEach((cleanup) => cleanup())
    vi.restoreAllMocks()
    vi.useRealTimers()
  })

  it("coalesces visibility, focus and pageshow into one frame for all panes", () => {
    const first = subscribe()
    const second = subscribe()
    visibilityChanged("hidden")
    vi.advanceTimersByTime(100)
    expect(first).not.toHaveBeenCalled()
    visibilityChanged("visible")
    window.dispatchEvent(new Event("focus"))
    window.dispatchEvent(new Event("pageshow"))
    expect(first).not.toHaveBeenCalled()
    vi.advanceTimersByTime(20)
    expect(first).toHaveBeenCalledOnce()
    expect(second).toHaveBeenCalledOnce()
  })

  it("does not repaint during ordinary clock checks", () => {
    const listener = subscribe()
    vi.advanceTimersByTime(60_000)
    expect(listener).not.toHaveBeenCalled()
  })

  it("detects sleep without a focus or visibility event", () => {
    const listener = subscribe()
    vi.setSystemTime(Date.now() + 60_000)
    vi.advanceTimersByTime(2_020)
    expect(listener).toHaveBeenCalledOnce()
    vi.advanceTimersByTime(20_000)
    expect(listener).toHaveBeenCalledOnce()
  })

  it("does not render a hidden document, even after a clock gap", () => {
    const listener = subscribe()
    visibilityChanged("hidden")
    vi.setSystemTime(Date.now() + 60_000)
    vi.advanceTimersByTime(2_020)
    expect(listener).not.toHaveBeenCalled()
    visibilityChanged("visible")
    vi.advanceTimersByTime(20)
    expect(listener).toHaveBeenCalledOnce()
  })

  it("rechecks visibility when a queued frame finally runs", () => {
    const listener = subscribe()
    window.dispatchEvent(new Event("focus"))
    visibilityChanged("hidden")
    vi.advanceTimersByTime(20)
    expect(listener).not.toHaveBeenCalled()
    visibilityChanged("visible")
    vi.advanceTimersByTime(20)
    expect(listener).toHaveBeenCalledOnce()
  })

  it("unsubscribes panes and cancels timers and pending frames after the last one", () => {
    const first = subscribe()
    const second = subscribe()
    cleanups.shift()!()
    window.dispatchEvent(new Event("focus"))
    vi.advanceTimersByTime(20)
    expect(first).not.toHaveBeenCalled()
    expect(second).toHaveBeenCalledOnce()
    window.dispatchEvent(new Event("focus"))
    cleanups.shift()!()
    expect(vi.getTimerCount()).toBe(0)
    vi.advanceTimersByTime(20)
    window.dispatchEvent(new Event("focus"))
    expect(vi.getTimerCount()).toBe(0)
    expect(second).toHaveBeenCalledOnce()
    const next = subscribe()
    window.dispatchEvent(new Event("focus"))
    vi.advanceTimersByTime(20)
    expect(next).toHaveBeenCalledOnce()
  })
})
