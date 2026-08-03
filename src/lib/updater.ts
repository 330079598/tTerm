import { getVersion } from "@tauri-apps/api/app"
import { Channel, invoke } from "@tauri-apps/api/core"
import { relaunch } from "@tauri-apps/plugin-process"

import { toErrorMessage } from "@/lib/utils"

export type UpdateChannel = "stable" | "beta-dev"
export type UpdateCheckFrequency = "daily" | "every-3-days" | "weekly" | "never"

export type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "not-available"
  | "downloading"
  | "downloaded"
  | "installing"
  | "ready"
  | "error"

export interface UpdateState {
  status: UpdateStatus
  channel: UpdateChannel
  currentVersion: string
  latestVersion?: string
  notes?: string
  error?: string
  errorCode?: "downloaded-update-unavailable"
  downloadedBytes: number
  totalBytes?: number
  canRetry?: boolean
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
let pendingUpdateChannel: UpdateChannel | null = null
let downloadedUpdateChannel: UpdateChannel | null = null
let hasInstalledUpdate = false
let checkInFlight: Promise<AppUpdateMetadata | null> | null = null
let downloadInFlight: Promise<boolean> | null = null
let installInFlight: Promise<boolean> | null = null
let downloadAndInstallInFlight: Promise<boolean> | null = null
let autoInstallRequested = false
let failedOperation: {
  action: "check" | "download" | "download-and-install" | "install"
  channel: UpdateChannel
} | null = null
let startupTimer: ReturnType<typeof setTimeout> | null = null
let intervalTimer: ReturnType<typeof setTimeout> | null = null
const listeners = new Set<UpdateStateListener>()

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

  if (downloadInFlight || installInFlight || downloadAndInstallInFlight) {
    return pendingUpdate
  }

  if (checkInFlight) {
    return checkInFlight
  }

  checkInFlight = (async () => {
    try {
      const currentVersion = await getVersion().catch(() => state.currentVersion)
      failedOperation = null
      publish({
        status: "checking",
        channel,
        currentVersion,
        error: undefined,
        errorCode: undefined,
        downloadedBytes: 0,
        totalBytes: undefined,
        canRetry: false,
      })

      const update = await invoke<AppUpdateMetadata | null>("check_app_update", { channel })
      pendingUpdate = update
      pendingUpdateChannel = update ? channel : null

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
      failedOperation = silent ? null : { action: "check", channel }
      publish({
        status: silent ? "idle" : "error",
        error: toErrorMessage(error),
        canRetry: !silent,
      })
      return null
    } finally {
      checkInFlight = null
    }
  })()

  return checkInFlight
}

async function runDownloadAppUpdate(channel: UpdateChannel, publishDownloaded: boolean) {
  if (downloadedUpdateChannel === channel || hasInstalledUpdate) {
    if (hasInstalledUpdate) {
      publish({ status: "ready" })
    } else if (publishDownloaded) {
      publish({ status: "downloaded" })
    }
    return true
  }

  if (downloadInFlight) {
    return downloadInFlight
  }

  downloadInFlight = (async () => {
    const update =
      pendingUpdateChannel === channel ? pendingUpdate : await checkForAppUpdate(channel, true)
    if (!update) {
      return false
    }

    try {
      let downloadedBytes = 0
      let totalBytes: number | undefined
      failedOperation = null
      publish({
        status: "downloading",
        channel,
        latestVersion: update.version,
        error: undefined,
        errorCode: undefined,
        downloadedBytes,
        totalBytes,
        canRetry: false,
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

      downloadedUpdateChannel = channel
      if (publishDownloaded && !autoInstallRequested) {
        publish({ status: "downloaded", downloadedBytes, totalBytes })
      }
      return true
    } catch (error) {
      failedOperation = {
        action: publishDownloaded && !autoInstallRequested ? "download" : "download-and-install",
        channel,
      }
      publish({ status: "error", error: toErrorMessage(error), canRetry: true })
      return false
    }
  })()

  try {
    return await downloadInFlight
  } finally {
    downloadInFlight = null
  }
}

export function downloadAppUpdate(channel: UpdateChannel) {
  return runDownloadAppUpdate(channel, true)
}

export async function installDownloadedAppUpdate(channel: UpdateChannel) {
  if (hasInstalledUpdate) {
    publish({ status: "ready" })
    return true
  }

  if (installInFlight) {
    return installInFlight
  }

  installInFlight = (async () => {
    try {
      failedOperation = null
      publish({
        status: "installing",
        channel,
        error: undefined,
        errorCode: undefined,
        canRetry: false,
      })
      const installed = await invoke<boolean>("install_downloaded_app_update", { channel })
      if (!installed) {
        downloadedUpdateChannel = null
        failedOperation = { action: "check", channel }
        publish({
          status: "error",
          error: undefined,
          errorCode: "downloaded-update-unavailable",
          canRetry: true,
        })
        return false
      }

      downloadedUpdateChannel = null
      hasInstalledUpdate = true
      pendingUpdate = null
      pendingUpdateChannel = null
      publish({ status: "ready", canRetry: false })
      return true
    } catch (error) {
      failedOperation = { action: "install", channel }
      publish({ status: "error", error: toErrorMessage(error), canRetry: true })
      return false
    }
  })()

  try {
    return await installInFlight
  } finally {
    installInFlight = null
  }
}

export async function downloadAndInstallAppUpdate(channel: UpdateChannel) {
  if (downloadAndInstallInFlight) {
    return downloadAndInstallInFlight
  }

  autoInstallRequested = true
  downloadAndInstallInFlight = (async () => {
    const downloaded = await runDownloadAppUpdate(channel, false)
    if (!downloaded) {
      return false
    }

    return installDownloadedAppUpdate(channel)
  })()

  try {
    return await downloadAndInstallInFlight
  } finally {
    downloadAndInstallInFlight = null
    autoInstallRequested = false
  }
}

export async function retryAppUpdate() {
  const operation = failedOperation
  if (!operation) {
    return false
  }

  switch (operation.action) {
    case "check":
      return Boolean(await checkForAppUpdate(operation.channel))
    case "download":
      return downloadAppUpdate(operation.channel)
    case "download-and-install":
      return downloadAndInstallAppUpdate(operation.channel)
    case "install":
      return installDownloadedAppUpdate(operation.channel)
  }
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
