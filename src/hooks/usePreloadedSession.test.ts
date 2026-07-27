// @vitest-environment jsdom

import { renderHook } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { usePreloadedSession } from "@/hooks/usePreloadedSession"

describe("usePreloadedSession", () => {
  it("starts loading on mount and reuses the same promise", async () => {
    const session = { activeTabId: "terminal-1" }
    const loadSession = vi.fn().mockResolvedValue(session)
    const { result, rerender } = renderHook(({ loader }) => usePreloadedSession(loader), {
      initialProps: { loader: loadSession },
    })

    expect(loadSession).toHaveBeenCalledTimes(1)

    const firstPromise = result.current()
    rerender({ loader: vi.fn().mockResolvedValue({ activeTabId: "terminal-2" }) })
    const secondPromise = result.current()

    expect(secondPromise).toBe(firstPromise)
    await expect(secondPromise).resolves.toBe(session)
    expect(loadSession).toHaveBeenCalledTimes(1)
  })
})
