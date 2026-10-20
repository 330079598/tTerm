// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react"
import { describe, expect, it } from "vitest"

import { getTabRemoteCwd, setTabRemoteCwd, useTabRemoteCwd } from "@/lib/remoteCwd"

describe("remote cwd store", () => {
  it("keeps the last directory per tab until its session is gone", () => {
    setTabRemoteCwd("tab-a", "/srv")
    setTabRemoteCwd("tab-b", "/tmp")
    setTabRemoteCwd("tab-a", "/var/log")
    expect(getTabRemoteCwd("tab-a")).toBe("/var/log")
    expect(getTabRemoteCwd("tab-b")).toBe("/tmp")

    setTabRemoteCwd("tab-a", null)
    expect(getTabRemoteCwd("tab-a")).toBeUndefined()
    setTabRemoteCwd("tab-b", null)
  })

  it("re-renders the tab's subscribers when its directory changes", () => {
    const { result } = renderHook(() => useTabRemoteCwd("tab-c"))
    expect(result.current).toBeUndefined()

    act(() => setTabRemoteCwd("tab-c", "/etc"))
    expect(result.current).toBe("/etc")

    act(() => setTabRemoteCwd("tab-c", null))
    expect(result.current).toBeUndefined()
  })
})
