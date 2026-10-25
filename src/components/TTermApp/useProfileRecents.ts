import { useCallback, useState } from "react"

import type { LauncherRecent } from "@/components/NewTabLauncher/launcherItems"
import type { AppShell } from "@/components/TTermApp/appShell"
import type { ConnectionState } from "@/components/TerminalTab/types"
import {
  addRecentProfile,
  loadRecentProfiles,
  removeRecentProfile,
  saveRecentProfiles,
  type RecentProfile,
} from "@/lib/profileRecents"
import type { RecentQuickConnection } from "@/lib/quickConnect"

type ConnectionStateChange = (
  tabId: string,
  sessionNonce: number,
  connectionState: ConnectionState | null
) => void

/** What the profile recents wrap: the handlers for everything else. */
interface ProfileRecentsDeps {
  onConnectionStateChange: ConnectionStateChange
  forgetQuickConnectRecent: (recent: RecentQuickConnection) => void
  clearQuickConnectRecents: () => void
}

/**
 * The saved connections used lately. A profile counts as used each time one
 * of its tabs connects, an SSH shell or a serial port alike, so a reconnect
 * moves it to the front as well. The launcher lists them with the quick
 * connections, so forgetting and clearing its recents covers both.
 */
export function useProfileRecents(
  shell: AppShell,
  {
    onConnectionStateChange,
    forgetQuickConnectRecent,
    clearQuickConnectRecents,
  }: ProfileRecentsDeps
) {
  const { tabsRef } = shell
  const [profileRecents, setProfileRecents] = useState<RecentProfile[]>(() =>
    typeof window === "undefined" ? [] : loadRecentProfiles(window.localStorage)
  )

  const updateProfileRecents = useCallback(
    (update: (recents: RecentProfile[]) => RecentProfile[]) => {
      setProfileRecents((current) => {
        const next = update(current)
        saveRecentProfiles(window.localStorage, next)
        return next
      })
    },
    []
  )

  const handleTerminalConnectionStateChange = useCallback<ConnectionStateChange>(
    (tabId, sessionNonce, connectionState) => {
      onConnectionStateChange(tabId, sessionNonce, connectionState)
      if (connectionState !== "connected") return
      const profileId = tabsRef.current.find((tab) => tab.id === tabId)?.connection?.profileId
      if (profileId) {
        updateProfileRecents((current) => addRecentProfile(current, profileId, Date.now()))
      }
    },
    [onConnectionStateChange, tabsRef, updateProfileRecents]
  )

  const handleForgetRecent = useCallback(
    (recent: LauncherRecent) => {
      if (recent.kind === "quick") {
        forgetQuickConnectRecent(recent.connection)
      } else {
        updateProfileRecents((current) => removeRecentProfile(current, recent.profileId))
      }
    },
    [forgetQuickConnectRecent, updateProfileRecents]
  )

  const handleClearRecents = useCallback(() => {
    clearQuickConnectRecents()
    updateProfileRecents(() => [])
  }, [clearQuickConnectRecents, updateProfileRecents])

  return {
    profileRecents,
    handleTerminalConnectionStateChange,
    handleForgetRecent,
    handleClearRecents,
  }
}
