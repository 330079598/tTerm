import { getVersion } from "@tauri-apps/api/app"
import { Channel, invoke } from "@tauri-apps/api/core"
import { relaunch } from "@tauri-apps/plugin-process"

export type UpdateChannel = "stable" | "beta-dev"
export type UpdateCheckFrequency = "daily" | "every-3-days" | "weekly" | "never"

export type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "not-available"
  | "downloading"
  | "downloaded"
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

const STARTUP_UPDATE_DELAY_MS = 6_000

const UPDATE_CHECK_INTERVALS_MS: Record<Exclude<UpdateCheckFrequency, "never">, number> = {
  daily: 24 * 60 * 60 * 1000,
  "every-3-days": 3 * 24 * 60 * 60 * 1000,
  weekly: 7 * 24 * 60 * 60 * 1000,
}

let state: UpdateState = {
  status: "idle",
  channel: "stable",
  currentVersion: import.meta.env.PACKAGE_VERSION ?? "0.0.0",
  downloadedBytes: 0,
}
let pendingUpdate: AppUpdateMetadata | null = null
let hasDownloadedUpdate = false
let hasInstalledUpdate = false
let checkInFlight: Promise<AppUpdateMetadata | null> | null = null
let startupTimer: ReturnType<typeof setTimeout> | null = null
let intervalTimer: ReturnType<typeof setTimeout> | null = null
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

export async function downloadAppUpdate(channel: UpdateChannel) {
  if (hasDownloadedUpdate || hasInstalledUpdate) {
    publish({ status: hasInstalledUpdate ? "ready" : "downloaded" })
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

    const downloaded = await invoke<boolean>("download_app_update", { channel, onEvent })
    if (!downloaded) {
      publish({ status: "not-available", latestVersion: undefined })
      return false
    }

    hasDownloadedUpdate = true
    publish({ status: "downloaded", downloadedBytes, totalBytes })
    return true
  } catch (error) {
    publish({ status: "error", error: toErrorMessage(error) })
    return false
  }
}

export async function installDownloadedAppUpdate(channel: UpdateChannel) {
  if (hasInstalledUpdate) {
    publish({ status: "ready" })
    return true
  }

  try {
    const installed = await invoke<boolean>("install_downloaded_app_update", { channel })
    if (!installed) {
      return false
    }

    hasDownloadedUpdate = false
    hasInstalledUpdate = true
    pendingUpdate = null
    publish({ status: "ready" })
    return true
  } catch (error) {
    publish({ status: "error", error: toErrorMessage(error) })
    return false
  }
}

export async function downloadAndInstallAppUpdate(channel: UpdateChannel) {
  const downloaded = await downloadAppUpdate(channel)
  if (!downloaded) {
    return false
  }

  return installDownloadedAppUpdate(channel)
}

export async function relaunchApp() {
  await relaunch()
}

export function startBackgroundUpdateChecks(
  channel: UpdateChannel,
  autoDownload: boolean,
  frequency: UpdateCheckFrequency,
  lastCheckedAt: number | null | undefined,
  onCheckComplete: (checkedAt: number) => void
) {
  stopBackgroundUpdateChecks()

  if (frequency === "never") {
    return
  }

  const intervalMs = UPDATE_CHECK_INTERVALS_MS[frequency]

  const run = async () => {
    const update = await checkForAppUpdate(channel, true)
    onCheckComplete(Date.now())
    if (update && autoDownload) {
      await downloadAppUpdate(channel)
    }
  }

  const scheduleNext = (delayMs: number) => {
    intervalTimer = setTimeout(() => {
      void (async () => {
        await run()
        scheduleNext(intervalMs)
      })()
    }, delayMs)
  }

  const elapsedMs = lastCheckedAt
    ? Math.max(0, Date.now() - lastCheckedAt)
    : Number.POSITIVE_INFINITY
  const isDue = elapsedMs >= intervalMs

  if (isDue) {
    startupTimer = setTimeout(() => {
      void (async () => {
        await run()
        scheduleNext(intervalMs)
      })()
    }, STARTUP_UPDATE_DELAY_MS)
    return
  }

  scheduleNext(intervalMs - elapsedMs)
}

export function stopBackgroundUpdateChecks() {
  if (startupTimer) {
    clearTimeout(startupTimer)
    startupTimer = null
  }

  if (intervalTimer) {
    clearTimeout(intervalTimer)
    intervalTimer = null
  }
}
