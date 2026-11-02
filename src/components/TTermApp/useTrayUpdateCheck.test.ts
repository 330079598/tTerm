// @vitest-environment jsdom

import { act, renderHook, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { useTrayUpdateCheck } from "@/components/TTermApp/useTrayUpdateCheck"
import { useTabs } from "@/hooks/useTabs"
import type { UpdateStatus } from "@/lib/updater"

const mocks = vi.hoisted(() => ({
  listen: vi.fn(),
  check: vi.fn(),
  saveAppState: vi.fn(),
  config: { update_channel: "stable", update_check_frequency: "never" },
  isLoaded: true,
  status: "idle" as UpdateStatus,
}))

vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }))
vi.mock("@/contexts/ConfigContext", () => ({
  useConfig: () => ({
    config: mocks.config,
    isLoaded: mocks.isLoaded,
    saveAppState: mocks.saveAppState,
  }),
}))
vi.mock("@/lib/updater", () => ({
  checkForAppUpdate: mocks.check,
  getUpdaterState: () => ({ status: mocks.status }),
}))

beforeEach(() => {
  mocks.listen.mockReset().mockResolvedValue(vi.fn())
  mocks.check.mockReset().mockResolvedValue(null)
  mocks.saveAppState.mockReset().mockResolvedValue(undefined)
  mocks.config.update_channel = "stable"
  mocks.status = "idle"
  mocks.isLoaded = true
})

function setup() {
  return renderHook(() => {
    const tabs = useTabs()
    useTrayUpdateCheck(tabs, "Settings")
    return tabs
  })
}

async function clickTray() {
  expect(mocks.listen).toHaveBeenCalledWith("tray-check-updates", expect.any(Function))
  const handler = mocks.listen.mock.calls[mocks.listen.mock.calls.length - 1][1] as () => void
  await act(async () => handler())
}

describe("tray update checks", () => {
  it("opens the update page and checks even when automatic checks are disabled", async () => {
    const { result } = setup()
    await clickTray()
    expect(result.current.tabs[0]).toMatchObject({
      type: "settings",
      settingsSection: "updates",
      isActive: true,
    })
    expect(mocks.check).toHaveBeenCalledWith("stable")
    expect(mocks.saveAppState).toHaveBeenCalledWith({ lastUpdateCheckAt: expect.any(Number) })
  })

  it("uses the latest configured channel and switches an existing settings page", async () => {
    const { result, rerender } = setup()
    act(() => result.current.openSettingsTab("Settings", "general"))
    mocks.config.update_channel = "beta-dev"
    rerender()
    await clickTray()
    expect(result.current.tabs).toHaveLength(1)
    expect(result.current.tabs[0].settingsSection).toBe("updates")
    expect(mocks.check).toHaveBeenCalledWith("beta-dev")
    expect(mocks.listen).toHaveBeenCalledTimes(1)
  })

  it.each(["downloading", "downloaded", "installing", "ready"] as const)(
    "shows the existing %s state without starting another check",
    async (status) => {
      mocks.status = status
      const { result } = setup()
      await clickTray()
      expect(result.current.tabs[0].settingsSection).toBe("updates")
      expect(mocks.check).not.toHaveBeenCalled()
      expect(mocks.saveAppState).not.toHaveBeenCalled()
    }
  )

  it("joins an existing check", async () => {
    mocks.status = "checking"
    setup()
    await clickTray()
    expect(mocks.check).toHaveBeenCalledWith("stable")
  })

  it("waits for config and removes a listener that resolves after unmount", async () => {
    mocks.isLoaded = false
    let finish!: (cleanup: () => void) => void
    mocks.listen.mockImplementation(() => new Promise((resolve) => (finish = resolve)))
    const { rerender, unmount } = setup()
    expect(mocks.listen).not.toHaveBeenCalled()
    mocks.isLoaded = true
    rerender()
    unmount()
    const cleanup = vi.fn()
    finish(cleanup)
    await waitFor(() => expect(cleanup).toHaveBeenCalledTimes(1))
  })
})
