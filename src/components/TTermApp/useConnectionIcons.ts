import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useCallback, useEffect } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import { detectDistroId, type LinuxDistributionInfo } from "@/lib/distro"
import { sameConnectionTabs } from "@/lib/profileConnections"
import type { TabIconStyle } from "@/lib/tabIcons"

interface RemoteOsPayload {
  tabId: string
  /** Null for a host without `/etc/os-release`, such as macOS or a BSD. */
  distribution: LinuxDistributionInfo | null
}

/**
 * Keeps the icons of SSH tabs: the distribution each host reports on
 * connecting, and the icon the user picks. Both belong to a saved
 * connection, so every open tab of it changes and the profile remembers.
 */
export function useConnectionIcons({ tabsRef, updateTab }: AppShell) {
  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void listen<RemoteOsPayload>("ssh-remote-os", (event) => {
      const { tabId, distribution } = event.payload
      const tab = tabsRef.current.find((candidate) => candidate.id === tabId)
      const distroId = distribution ? detectDistroId(distribution) : undefined
      if (tab?.type !== "ssh" || tab.distroId === distroId) return
      for (const other of sameConnectionTabs(tabsRef.current, tab)) {
        updateTab(other.id, (current) => ({ ...current, distroId }))
      }
      const profileId = tab.connection?.profileId
      if (profileId) {
        invoke("set_profile_distro", { id: profileId, distro: distroId ?? null }).catch((error) => {
          console.error("Failed to save the host's distribution:", error)
        })
      }
    }).then((cleanup) => {
      if (disposed) cleanup()
      else unlisten = cleanup
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [tabsRef, updateTab])

  /** Gives a tab its own icon, or every tab of its saved SSH connection. */
  const setTabIconStyle = useCallback(
    (tabId: string, iconStyle: TabIconStyle | undefined) => {
      const tab = tabsRef.current.find((candidate) => candidate.id === tabId)
      if (!tab) return
      for (const other of sameConnectionTabs(tabsRef.current, tab)) {
        updateTab(other.id, (current) => ({ ...current, iconStyle }))
      }
      const profileId = tab.type === "ssh" ? tab.connection?.profileId : undefined
      if (profileId) {
        invoke("set_profile_tab_icon", { id: profileId, icon: iconStyle ?? null }).catch(
          (error) => {
            console.error("Failed to save the connection's icon:", error)
          }
        )
      }
    },
    [tabsRef, updateTab]
  )

  return { setTabIconStyle }
}
