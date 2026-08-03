import { beforeEach, describe, expect, it, vi } from "vitest"

const mocks = vi.hoisted(() => ({
  getVersion: vi.fn(),
  invoke: vi.fn(),
  relaunch: vi.fn(),
}))

vi.mock("@tauri-apps/api/app", () => ({ getVersion: mocks.getVersion }))
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage?: (event: unknown) => void
  },
  invoke: mocks.invoke,
}))
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }))

const update = {
  version: "1.2.3",
  currentVersion: "1.0.0",
  body: "Release notes",
  date: null,
}

async function loadUpdater() {
  return import("@/lib/updater")
}

beforeEach(() => {
  vi.resetModules()
  mocks.getVersion.mockReset().mockResolvedValue("1.0.0")
  mocks.relaunch.mockReset().mockResolvedValue(undefined)
  mocks.invoke.mockReset().mockImplementation(async (command: string) => {
    if (command === "check_app_update") return update
    if (command === "download_app_update") return true
    if (command === "install_downloaded_app_update") return true
    throw new Error(`Unexpected command: ${command}`)
  })
})

describe("app updater state transitions", () => {
  it("does not publish the downloaded confirmation state for download and install", async () => {
    const updater = await loadUpdater()
    await updater.checkForAppUpdate("stable")
    const statuses: string[] = []
    const unsubscribe = updater.subscribeToUpdater((state) => statuses.push(state.status))

    await updater.downloadAndInstallAppUpdate("stable")

    expect(statuses).toEqual(["available", "downloading", "installing", "ready"])
    expect(statuses).not.toContain("downloaded")
    unsubscribe()
  })

  it("keeps background downloads in the downloaded state until the user installs", async () => {
    const updater = await loadUpdater()
    await updater.checkForAppUpdate("stable")
    const statuses: string[] = []
    const unsubscribe = updater.subscribeToUpdater((state) => statuses.push(state.status))

    await updater.downloadAppUpdate("stable")

    expect(statuses).toEqual(["available", "downloading", "downloaded"])
    expect(mocks.invoke).not.toHaveBeenCalledWith(
      "install_downloaded_app_update",
      expect.anything()
    )
    unsubscribe()
  })

  it("coalesces repeated download and install requests", async () => {
    const updater = await loadUpdater()
    await updater.checkForAppUpdate("stable")

    const [first, second] = await Promise.all([
      updater.downloadAndInstallAppUpdate("stable"),
      updater.downloadAndInstallAppUpdate("stable"),
    ])

    expect(first).toBe(true)
    expect(second).toBe(true)
    expect(
      mocks.invoke.mock.calls.filter(([command]) => command === "download_app_update")
    ).toHaveLength(1)
    expect(
      mocks.invoke.mock.calls.filter(([command]) => command === "install_downloaded_app_update")
    ).toHaveLength(1)
  })

  it("suppresses the downloaded state when a user takes over a background download", async () => {
    const updater = await loadUpdater()
    await updater.checkForAppUpdate("stable")
    let finishDownload: ((value: boolean) => void) | undefined
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "download_app_update") {
        return new Promise<boolean>((resolve) => {
          finishDownload = resolve
        })
      }
      if (command === "install_downloaded_app_update") return Promise.resolve(true)
      throw new Error(`Unexpected command: ${command}`)
    })
    const statuses: string[] = []
    const unsubscribe = updater.subscribeToUpdater((state) => statuses.push(state.status))

    const backgroundDownload = updater.downloadAppUpdate("stable")
    await vi.waitFor(() => expect(finishDownload).toBeTypeOf("function"))
    const userInstall = updater.downloadAndInstallAppUpdate("stable")
    finishDownload?.(true)
    await Promise.all([backgroundDownload, userInstall])

    expect(statuses).toEqual(["available", "downloading", "installing", "ready"])
    unsubscribe()
  })

  it("retries installation intent after a taken-over background download fails", async () => {
    const updater = await loadUpdater()
    await updater.checkForAppUpdate("stable")
    let rejectDownload: ((reason: Error) => void) | undefined
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "download_app_update") {
        return new Promise<boolean>((_resolve, reject) => {
          rejectDownload = reject
        })
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    const backgroundDownload = updater.downloadAppUpdate("stable")
    await vi.waitFor(() => expect(rejectDownload).toBeTypeOf("function"))
    const userInstall = updater.downloadAndInstallAppUpdate("stable")
    rejectDownload?.(new Error("network unavailable"))
    await Promise.all([backgroundDownload, userInstall])

    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === "download_app_update") return true
      if (command === "install_downloaded_app_update") return true
      throw new Error(`Unexpected command: ${command}`)
    })
    await expect(updater.retryAppUpdate()).resolves.toBe(true)
    expect(updater.getUpdaterState().status).toBe("ready")
  })

  it("exposes a retry action after an install failure", async () => {
    const updater = await loadUpdater()
    await updater.checkForAppUpdate("stable")
    await updater.downloadAppUpdate("stable")
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === "install_downloaded_app_update") {
        throw new Error("installer unavailable")
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    await expect(updater.installDownloadedAppUpdate("stable")).resolves.toBe(false)
    expect(updater.getUpdaterState()).toMatchObject({
      status: "error",
      error: "installer unavailable",
      canRetry: true,
    })
  })

  it("downloads again after the backend discards an unavailable cached update", async () => {
    const updater = await loadUpdater()
    await updater.checkForAppUpdate("stable")
    await updater.downloadAppUpdate("stable")
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === "install_downloaded_app_update") return false
      if (command === "check_app_update") return update
      if (command === "download_app_update") return true
      throw new Error(`Unexpected command: ${command}`)
    })

    await expect(updater.installDownloadedAppUpdate("stable")).resolves.toBe(false)
    expect(updater.getUpdaterState()).toMatchObject({
      status: "error",
      errorCode: "downloaded-update-unavailable",
      canRetry: true,
    })
    await expect(updater.retryAppUpdate()).resolves.toBe(true)
    await expect(updater.downloadAppUpdate("stable")).resolves.toBe(true)

    expect(
      mocks.invoke.mock.calls.filter(([command]) => command === "download_app_update")
    ).toHaveLength(2)
  })

  it("does not reuse update metadata from a different channel", async () => {
    const updater = await loadUpdater()
    await updater.checkForAppUpdate("stable")
    mocks.invoke.mockImplementation(async (command: string, args?: { channel?: string }) => {
      if (command === "check_app_update") {
        return { ...update, version: args?.channel === "beta-dev" ? "2.0.0-beta.1" : "1.2.3" }
      }
      if (command === "download_app_update") return true
      throw new Error(`Unexpected command: ${command}`)
    })

    await updater.downloadAppUpdate("beta-dev")

    expect(mocks.invoke).toHaveBeenCalledWith("check_app_update", { channel: "beta-dev" })
    expect(updater.getUpdaterState()).toMatchObject({
      channel: "beta-dev",
      latestVersion: "2.0.0-beta.1",
      status: "downloaded",
    })
  })
})
