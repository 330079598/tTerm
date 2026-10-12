import { invoke } from "@tauri-apps/api/core"
import { useCallback } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"

/**
 * Actions on a tab's connection: reconnecting, pinning its header, and
 * showing its server monitor.
 */
export function useTabConnectionActions(shell: AppShell) {
  const { tabs, updateTab } = shell

  const handleReconnectTab = useCallback(
    (tabId: string) => {
      updateTab(tabId, (tab) => ({
        ...tab,
        sessionNonce: ((tab.sessionNonce ?? 0) + 1) >>> 0,
      }))
    },
    [updateTab]
  )

  const handlePinConnectionHeader = useCallback(
    (tabId: string) => {
      updateTab(tabId, (tab) => ({
        ...tab,
        connectionHeaderPinned: true,
      }))
    },
    [updateTab]
  )

  const handleUnpinConnectionHeader = useCallback(
    (tabId: string) => {
      updateTab(tabId, (tab) => ({
        ...tab,
        connectionHeaderPinned: false,
      }))
    },
    [updateTab]
  )

  const handleServerMonitorVisibilityChange = useCallback(
    (tabId: string, visible: boolean) => {
      const profileId = tabs.find((tab) => tab.id === tabId)?.connection?.profileId

      updateTab(tabId, (tab) => {
        if (!tab.connection || tab.connection.type !== "ssh") {
          return tab
        }

        return {
          ...tab,
          connection: {
            ...tab.connection,
            serverMonitorVisible: visible,
          },
        }
      })

      if (!profileId) {
        return
      }

      invoke("set_profile_server_monitor_visible", { id: profileId, visible }).catch((error) => {
        console.error("Failed to save server monitor preference:", error)
      })
    },
    [tabs, updateTab]
  )

  return {
    handleReconnectTab,
    handlePinConnectionHeader,
    handleUnpinConnectionHeader,
    handleServerMonitorVisibilityChange,
  }
}
