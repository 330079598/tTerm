import { platform } from "@tauri-apps/plugin-os"
import { useCallback, useEffect, useState } from "react"

import { useConfig } from "@/contexts/ConfigContext"
import { onSyncApplied } from "@/lib/sync"

function getRuntimePlatform() {
  try {
    return platform()
  } catch {
    return "windows"
  }
}

/** The OS the app runs on, read once. */
export function useRuntimePlatform() {
  const [os] = useState<string>(getRuntimePlatform)
  return { isMacos: os === "macos", isLinux: os === "linux", isWindows: os === "windows" }
}

/** Saves which host groups are collapsed in the host lists. */
export function useCollapsedProfileGroupsSaver() {
  const { saveAppState } = useConfig()
  return useCallback(
    (groups: string[]) => {
      saveAppState({ collapsedProfileGroupKeys: groups }).catch((error) => {
        console.error("Failed to save collapsed profile groups:", error)
      })
    },
    [saveAppState]
  )
}

/**
 * A counter the host lists reload on: bumped when a sync brings in hosts or
 * groups, and by `refreshProfiles`.
 */
export function useProfilesRefreshKey() {
  const [profilesRefreshKey, setProfilesRefreshKey] = useState(0)
  const refreshProfiles = useCallback(() => setProfilesRefreshKey((key) => key + 1), [])
  useEffect(() => onSyncApplied(["profiles", "profileGroups"], refreshProfiles), [refreshProfiles])
  return { profilesRefreshKey, refreshProfiles }
}

/** Keeps the webview's own context menu away, except where it is allowed. */
export function useBlockNativeContextMenu() {
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement
      if (!target.closest("[data-allow-context-menu]")) {
        e.preventDefault()
      }
    }
    document.addEventListener("contextmenu", handler)
    return () => document.removeEventListener("contextmenu", handler)
  }, [])
}
