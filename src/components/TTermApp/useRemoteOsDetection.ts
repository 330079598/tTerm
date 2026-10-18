import { listen } from "@tauri-apps/api/event"
import { useEffect } from "react"

import { detectDistroId, type LinuxDistributionInfo } from "@/lib/distro"
import type { Tab } from "@/types/tab"

interface RemoteOsPayload {
  tabId: string
  /** Null for a host without `/etc/os-release`, such as macOS or a BSD. */
  distribution: LinuxDistributionInfo | null
}

/**
 * Keeps each SSH tab's distribution up to date with what its host reports
 * on connecting, for the tab's icon.
 */
export function useRemoteOsDetection(updateTab: (id: string, updater: (tab: Tab) => Tab) => void) {
  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void listen<RemoteOsPayload>("ssh-remote-os", (event) => {
      const { tabId, distribution } = event.payload
      const distroId = distribution ? detectDistroId(distribution) : undefined
      updateTab(tabId, (tab) =>
        tab.type !== "ssh" || tab.distroId === distroId ? tab : { ...tab, distroId }
      )
    }).then((cleanup) => {
      if (disposed) cleanup()
      else unlisten = cleanup
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [updateTab])
}
