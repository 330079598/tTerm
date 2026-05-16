import { getVersion } from "@tauri-apps/api/app"
import { Channel, invoke } from "@tauri-apps/api/core"
import { relaunch } from "@tauri-apps/plugin-process"

export type UpdateChannel = "stable" | "beta-dev"

export type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "not-available"
  | "downloading"
  | "ready"
  | "error"

export interface UpdateState {
  status: UpdateStatus
  channel: UpdateChannel
  currentVersion: string
  latestVersion?: string
  notes?: string
  error?: string
  downloadedBytes: number
  totalBytes?: number
}

export interface AppUpdateMetadata {
  version: string
  currentVersion: string
  body?: string | null
  date?: string | null
}

export type DownloadEvent =
  | { event: "Started"; data: { contentLength?: number | null } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" }

export type UpdateStateListener = (state: UpdateState) => void

const UPDATE_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000
const STARTUP_UPDATE_DELAY_MS = 6_000

let state: UpdateState = {
  status: "idle",
  channel: "stable",
  currentVersion: import.meta.env.PACKAGE_VERSION ?? "0.0.0",
  downloadedBytes: 0,
}
let pendingUpdate: AppUpdateMetadata | null = null
let hasInstalledUpdate = false
let checkInFlight: Promise<AppUpdateMetadata | null> | null = null
let startupTimer: ReturnType<typeof setTimeout> | null = null
let intervalTimer: ReturnType<typeof setInterval> | null = null
const listeners = new Set<UpdateStateListener>()

function toErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

function publish(nextState: Partial<UpdateState>) {
  state = { ...state, ...nextState }
  listeners.forEach((listener) => listener(state))
}

export function subscribeToUpdater(listener: UpdateStateListener) {
  listeners.add(listener)
  listener(state)
  return () => listeners.delete(listener)
}

export function getUpdaterState() {
  return state
}

export async function checkForAppUpdate(channel: UpdateChannel, silent = false) {
  if (hasInstalledUpdate) {
    return pendingUpdate
  }

  if (checkInFlight) {
    return checkInFlight
  }

  checkInFlight = (async () => {
    try {
      const currentVersion = await getVersion().catch(() => state.currentVersion)
      publish({
        status: "checking",
        channel,
        currentVersion,
        error: undefined,
        downloadedBytes: 0,
        totalBytes: undefined,
      })

      const update = await invoke<AppUpdateMetadata | null>("check_app_update", { channel })
      pendingUpdate = update

      if (!update) {
        if (!silent) {
          publish({ status: "not-available", latestVersion: undefined, notes: undefined })
        } else {
          publish({ status: "idle", latestVersion: undefined, notes: undefined })
        }
        return null
      }

      publish({
        status: "available",
        latestVersion: update.version,
        notes: update.body ?? undefined,
        downloadedBytes: 0,
        totalBytes: undefined,
      })
      return update
    } catch (error) {
      publish({ status: silent ? "idle" : "error", error: toErrorMessage(error) })
      return null
    } finally {
      checkInFlight = null
    }
  })()

  return checkInFlight
}

export async function downloadAndInstallAppUpdate(channel: UpdateChannel) {
  if (hasInstalledUpdate) {
    publish({ status: "ready" })
    return true
  }

  const update = pendingUpdate ?? (await checkForAppUpdate(channel, true))
  if (!update) {
    return false
  }

  try {
    let downloadedBytes = 0
    let totalBytes: number | undefined
    publish({
      status: "downloading",
      channel,
      latestVersion: update.version,
      error: undefined,
      downloadedBytes,
      totalBytes,
    })

    const onEvent = new Channel<DownloadEvent>()
    onEvent.onmessage = (event) => {
      if (event.event === "Started") {
        totalBytes = event.data.contentLength ?? undefined
        publish({ downloadedBytes, totalBytes })
        return
      }

      if (event.event === "Progress") {
        downloadedBytes += event.data.chunkLength
        publish({ downloadedBytes, totalBytes })
        return
      }

      publish({ downloadedBytes, totalBytes })
    }

    const installed = await invoke<boolean>("download_install_app_update", { channel, onEvent })
    if (!installed) {
      publish({ status: "not-available", latestVersion: undefined })
      return false
    }

    hasInstalledUpdate = true
    pendingUpdate = null
    publish({ status: "ready", downloadedBytes, totalBytes })
    return true
  } catch (error) {
    publish({ status: "error", error: toErrorMessage(error) })
    return false
  }
}

export async function relaunchApp() {
  await relaunch()
}

export function startBackgroundUpdateChecks(channel: UpdateChannel, autoDownload: boolean) {
  stopBackgroundUpdateChecks()

  const run = async () => {
    const update = await checkForAppUpdate(channel, true)
    if (update && autoDownload) {
      await downloadAndInstallAppUpdate(channel)
    }
  }

  startupTimer = setTimeout(() => {
    void run()
  }, STARTUP_UPDATE_DELAY_MS)

  intervalTimer = setInterval(() => {
    void run()
  }, UPDATE_CHECK_INTERVAL_MS)
}

export function stopBackgroundUpdateChecks() {
  if (startupTimer) {
    clearTimeout(startupTimer)
    startupTimer = null
  }

  if (intervalTimer) {
    clearInterval(intervalTimer)
    intervalTimer = null
  }
}
