import { invoke } from "@tauri-apps/api/core"
import { useEffect, useState } from "react"

import { runSync, SYNC_SETTINGS_EVENT, type SyncSettings } from "@/lib/sync"

/** Coming back to the window checks the server, at most this often. */
const FOCUS_CHECK_INTERVAL_MS = 60_000
const MAX_RETRY_DELAY_MS = 5 * 60_000
/** Timers fire a little early or late; a tick this close to due counts as due. */
const TIMER_SLACK_MS = 2_000

interface Schedule {
  pushMs: number
  pullMs: number
}

/** Keeps this device in sync while the main window is open. */
export function useWebDavSync() {
  // `null` while sync is off: nothing is scheduled then.
  const [schedule, setSchedule] = useState<Schedule | null>(null)

  useEffect(() => {
    const load = () => {
      invoke<{ settings: SyncSettings }>("get_sync_status")
        .then(({ settings }) =>
          setSchedule((current) => {
            if (!settings.enabled) return null
            const next = {
              pushMs: settings.pushIntervalSecs * 1_000,
              pullMs: settings.pullIntervalMins * 60_000,
            }
            return current?.pushMs === next.pushMs && current.pullMs === next.pullMs
              ? current
              : next
          })
        )
        .catch((error) => console.error("Failed to load sync settings:", error))
    }
    load()
    window.addEventListener(SYNC_SETTINGS_EVENT, load)
    return () => window.removeEventListener(SYNC_SETTINGS_EVENT, load)
  }, [])

  useEffect(() => {
    if (!schedule) return
    const { pushMs, pullMs } = schedule
    let running = false
    let lastLocalCheck = 0
    let lastRemoteCheck = 0
    let failures = 0
    let retryAt = 0

    const tick = async (forceRemote: boolean) => {
      const now = Date.now()
      if (running || now < retryAt) return
      const checkRemote = forceRemote || now - lastRemoteCheck >= pullMs - TIMER_SLACK_MS
      if (!checkRemote && now - lastLocalCheck < pushMs - TIMER_SLACK_MS) return
      running = true
      lastLocalCheck = now
      try {
        const outcome = await runSync(checkRemote)
        if (checkRemote && outcome.state === "synced") lastRemoteCheck = now
        failures = 0
        retryAt = 0
      } catch (error) {
        // Offline or a server error: back off instead of retrying every tick.
        failures += 1
        retryAt = Date.now() + Math.min(MAX_RETRY_DELAY_MS, pushMs * 2 ** failures)
        console.error("WebDAV sync failed:", error)
      } finally {
        running = false
      }
    }

    // Local changes are looked for every `pushMs`, which costs no network
    // request; the server is asked every `pullMs`. Ticking at the shorter of
    // the two serves both.
    const startup = window.setTimeout(() => void tick(true), 3_000)
    const interval = window.setInterval(() => void tick(false), Math.min(pushMs, pullMs))
    const onFocus = () => {
      if (Date.now() - lastRemoteCheck >= FOCUS_CHECK_INTERVAL_MS) void tick(true)
    }
    window.addEventListener("focus", onFocus)
    return () => {
      window.clearTimeout(startup)
      window.clearInterval(interval)
      window.removeEventListener("focus", onFocus)
    }
  }, [schedule])
}
