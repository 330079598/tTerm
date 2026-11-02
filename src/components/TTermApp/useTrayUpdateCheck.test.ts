// @vitest-environment jsdom

import { act, renderHook, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { useTrayUpdateCheck } from "@/components/TTermApp/useTrayUpdateCheck"
import { useTabs } from "@/hooks/useTabs"
import type { UpdateStatus } from "@/lib/updater"

const mocks = vi.hoisted(() => ({
  listen: vi.fn(),
  invoke: vi.fn(),
  check: vi.fn(),
  saveAppState: vi.fn(),
  config: { update_channel: "stable", update_check_frequency: "never" },
  isLoaded: true,
  status: "idle" as UpdateStatus,
  requested: false,
}))

vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }))
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }))
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
  // Stands in for the backend, which hands a kept click out once.
  mocks.invoke.mockReset().mockImplementation(async (command: string) => {
    expect(command).toBe("take_tray_update_check")
    const requested = mocks.requested
    mocks.requested = false
    return requested
  })
  mocks.check.mockReset().mockResolvedValue(null)
  mocks.saveAppState.mockReset().mockResolvedValue(undefined)
  mocks.config.update_channel = "stable"
  mocks.status = "idle"
  mocks.isLoaded = true
  mocks.requested = false
})

async function setup() {
  const hook = renderHook(() => {
    const tabs = useTabs()
    useTrayUpdateCheck(tabs, "Settings")
    return tabs
  })
  // Listening takes whatever click was kept before it started.
  await waitFor(() => expect(mocks.invoke).toHaveBeenCalledTimes(1))
  return hook
}

async function clickTray() {
  expect(mocks.listen).toHaveBeenCalledWith("tray-check-updates", expect.any(Function))
  const handler = mocks.listen.mock.calls[mocks.listen.mock.calls.length - 1][1] as () => void
  const takes = mocks.invoke.mock.calls.length
  mocks.requested = true
  await act(async () => handler())
  await waitFor(() => expect(mocks.invoke).toHaveBeenCalledTimes(takes + 1))
}

describe("tray update checks", () => {
  it("opens the update page and checks even when automatic checks are disabled", async () => {
    const { result } = await setup()
    await clickTray()
    await waitFor(() => expect(mocks.check).toHaveBeenCalledWith("stable"))
    expect(result.current.tabs[0]).toMatchObject({
      type: "settings",
      settingsSection: "updates",
      isActive: true,
    })
    await waitFor(() =>
      expect(mocks.saveAppState).toHaveBeenCalledWith({ lastUpdateCheckAt: expect.any(Number) })
    )
  })

  it("uses the latest configured channel and switches an existing settings page", async () => {
    const { result, rerender } = await setup()
    act(() => result.current.openSettingsTab("Settings", "general"))
    mocks.config.update_channel = "beta-dev"
    rerender()
    await clickTray()
    await waitFor(() => expect(mocks.check).toHaveBeenCalledWith("beta-dev"))
    expect(result.current.tabs).toHaveLength(1)
    expect(result.current.tabs[0].settingsSection).toBe("updates")
    expect(mocks.listen).toHaveBeenCalledTimes(1)
  })

  it.each(["downloading", "downloaded", "installing", "ready"] as const)(
    "shows the existing %s state without starting another check",
    async (status) => {
      mocks.status = status
      const { result } = await setup()
      await clickTray()
      await waitFor(() => expect(result.current.tabs[0]?.settingsSection).toBe("updates"))
      expect(mocks.check).not.toHaveBeenCalled()
      expect(mocks.saveAppState).not.toHaveBeenCalled()
    }
  )

  it("joins an existing check", async () => {
    mocks.status = "checking"
    await setup()
    await clickTray()
    await waitFor(() => expect(mocks.check).toHaveBeenCalledWith("stable"))
  })

  it("runs a click made before it listened once the config has loaded", async () => {
    mocks.isLoaded = false
    mocks.requested = true
    const { rerender } = await setup()
    expect(mocks.check).not.toHaveBeenCalled()

    mocks.isLoaded = true
    rerender()
    await waitFor(() => expect(mocks.check).toHaveBeenCalledTimes(1))
    rerender()
    expect(mocks.check).toHaveBeenCalledTimes(1)
  })

  it("does nothing when told about a click that was already taken", async () => {
    mocks.requested = true
    await setup()
    await clickTray()
    await waitFor(() => expect(mocks.check).toHaveBeenCalledTimes(2))
    mocks.invoke.mockClear()
    const handler = mocks.listen.mock.calls[0][1] as () => void
    await act(async () => handler())
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledTimes(1))
    expect(mocks.check).toHaveBeenCalledTimes(2)
  })

  it("removes a listener that resolves after unmount", async () => {
    let finish!: (cleanup: () => void) => void
    mocks.listen.mockImplementation(() => new Promise((resolve) => (finish = resolve)))
    const { unmount } = renderHook(() => useTrayUpdateCheck(useTabs(), "Settings"))
    unmount()
    const cleanup = vi.fn()
    finish(cleanup)
    await waitFor(() => expect(cleanup).toHaveBeenCalledTimes(1))
    expect(mocks.invoke).not.toHaveBeenCalled()
  })
})
