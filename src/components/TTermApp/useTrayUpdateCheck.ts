import { listen } from "@tauri-apps/api/event"
import { useEffect, useRef } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import { useConfig } from "@/contexts/ConfigContext"
import { useLatestRef } from "@/hooks/useLatestRef"
import { checkForAppUpdate, getUpdaterState } from "@/lib/updater"

/** Opens the update page before checking so every result is visible. */
export function useTrayUpdateCheck(
  shell: Pick<AppShell, "openSettingsTab">,
  settingsTabTitle: string
) {
  const { config, isLoaded, saveAppState } = useConfig()
  const handleCheck = async () => {
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

  useEffect(() => {
    if (!isLoaded) return
    let disposed = false
    let unlisten: (() => void) | undefined
    void listen("tray-check-updates", () => void handleCheckRef.current())
      .then((cleanup) => {
        if (disposed) cleanup()
        else unlisten = cleanup
      })
      .catch((error) => {
        console.error("Failed to listen for tray update checks:", error)
      })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [isLoaded])
}
