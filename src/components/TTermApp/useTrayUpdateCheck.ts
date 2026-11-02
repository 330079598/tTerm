import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useEffect, useRef } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import { useConfig } from "@/contexts/ConfigContext"
import { useLatestRef } from "@/hooks/useLatestRef"
import { checkForAppUpdate, getUpdaterState } from "@/lib/updater"

/**
 * Runs the update checks asked for from the tray menu. The backend keeps a
 * click until it is taken, so one made before this listens is taken as soon
 * as it does, and it waits for the config, which names the channel.
 */
export function useTrayUpdateCheck(
  shell: Pick<AppShell, "openSettingsTab">,
  settingsTabTitle: string
) {
  const { config, isLoaded, saveAppState } = useConfig()
  const handleCheck = async () => {
    // The update page is opened first so every result is in sight.
    shell.openSettingsTab(settingsTabTitle, "updates")
    const { status } = getUpdaterState()
    if (["downloading", "downloaded", "installing", "ready"].includes(status)) return

    await checkForAppUpdate(config.update_channel)
    await saveAppState({ lastUpdateCheckAt: Date.now() }).catch((error) => {
      console.error("Failed to save update check timestamp:", error)
    })
  }
  const handleCheckRef = useRef(handleCheck)
  useLatestRef(handleCheckRef, handleCheck)
  const isLoadedRef = useRef(isLoaded)
  useLatestRef(isLoadedRef, isLoaded)
  const pendingRef = useRef(false)

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    const take = () => {
      invoke<boolean>("take_tray_update_check")
        .then((requested) => {
          if (disposed || !requested) return
          if (isLoadedRef.current) void handleCheckRef.current()
          else pendingRef.current = true
        })
        .catch((error) => {
          console.error("Failed to take a tray update check:", error)
        })
    }
    void listen("tray-check-updates", take)
      .then((cleanup) => {
        if (disposed) return cleanup()
        unlisten = cleanup
        take()
      })
      .catch((error) => {
        console.error("Failed to listen for tray update checks:", error)
      })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    if (!isLoaded || !pendingRef.current) return
    pendingRef.current = false
    void handleCheckRef.current()
  }, [isLoaded])
}
