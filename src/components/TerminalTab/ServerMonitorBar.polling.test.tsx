// @vitest-environment jsdom

import { act, cleanup, render } from "@testing-library/react"
import { invoke } from "@tauri-apps/api/core"
import type { TFunction } from "i18next"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { ServerMonitorBar } from "@/components/TerminalTab/ServerMonitorBar"
import type { TerminalTabProps } from "@/components/TerminalTab/types"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => ({})) }))
vi.mock("@/contexts/ConfigContext", () => ({
  useConfig: () => ({
    config: { monitor_refresh_interval_secs: 5, monitor_visible_metrics: [] },
  }),
}))
vi.mock("@/hooks/use-toast", () => ({ useToast: () => ({ toast: vi.fn() }) }))

const mockedInvoke = vi.mocked(invoke)
const t = ((key: string) => key) as unknown as TFunction
const connection = { type: "ssh", host: "example.com" } as TerminalTabProps["connection"]

function renderBar(active: boolean) {
  const props = { connection, connectionState: "connected", sessionNonce: 1, tabId: "tab-1", t }
  const view = render(<ServerMonitorBar {...props} active={active} visible />)
  return {
    setActive: (next: boolean) =>
      view.rerender(<ServerMonitorBar {...props} active={next} visible />),
  }
}

function snapshotCalls() {
  return mockedInvoke.mock.calls.filter(([command]) => command === "get_server_metrics_snapshot")
    .length
}

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.clearAllMocks()
})

describe("ServerMonitorBar polling", () => {
  it("waits until a tab restored in the background is shown", async () => {
    const bar = renderBar(false)
    await act(() => vi.advanceTimersByTimeAsync(30_000))
    expect(snapshotCalls()).toBe(0)

    bar.setActive(true)
    await act(() => vi.advanceTimersByTimeAsync(0))
    expect(snapshotCalls()).toBe(1)
  })

  it("stops polling out of view without closing the monitor connection", async () => {
    const bar = renderBar(true)
    await act(() => vi.advanceTimersByTimeAsync(0))
    expect(snapshotCalls()).toBe(1)

    bar.setActive(false)
    await act(() => vi.advanceTimersByTimeAsync(60_000))
    // At most the poll already scheduled when the tab was hidden runs.
    const whileHidden = snapshotCalls()
    expect(whileHidden).toBeLessThanOrEqual(2)
    expect(mockedInvoke).not.toHaveBeenCalledWith(
      "release_server_monitor_session",
      expect.anything()
    )

    bar.setActive(true)
    await act(() => vi.advanceTimersByTimeAsync(0))
    expect(snapshotCalls()).toBe(whileHidden + 1)
  })
})
