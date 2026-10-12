import { useCallback, useState } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { ProfileDialogState } from "@/components/TTermApp/useProfileDialogState"
import { SavedProfile } from "@/components/ProfilesPanel"
import { ToastAction } from "@/components/ui/toast"
import { toast } from "@/hooks/use-toast"
import { buildConnectionFromProfile } from "@/lib/profileConnections"
import {
  addRecentQuickConnection,
  applyQuickConnectAuth,
  draftProfileFromInput,
  draftProfileFromTab,
  loadRecentQuickConnections,
  quickConnectionFromTab,
  removeRecentQuickConnection,
  saveRecentQuickConnections,
  type RecentQuickConnection,
  type SshConfigHost,
} from "@/lib/quickConnect"
import { Tab } from "@/types/tab"
import type { SshConnectionProgress } from "@/components/TerminalTab/types"

/**
 * Quick connections: the recent ones, opening the profile dialog for one,
 * and offering to save one as a profile once it connects.
 */
export function useQuickConnect(shell: AppShell, dialog: ProfileDialogState) {
  const { t, tabsRef, updateTab } = shell
  const {
    setShowConnectionDialog,
    setDraftProfile,
    savingQuickConnectTabId,
    setSavingQuickConnectTabId,
    setEditingProfile,
    setDuplicatingProfile,
  } = dialog
  const [quickConnectRecents, setQuickConnectRecents] = useState<RecentQuickConnection[]>(() =>
    typeof window === "undefined" ? [] : loadRecentQuickConnections(window.localStorage)
  )

  const handleOpenConnectionDialogFromLauncher = useCallback(
    (input: string, configHosts: readonly SshConfigHost[]) => {
      setEditingProfile(null)
      setDuplicatingProfile(null)
      setDraftProfile(draftProfileFromInput(input, configHosts))
      setShowConnectionDialog(true)
    },
    [setDraftProfile, setDuplicatingProfile, setEditingProfile, setShowConnectionDialog]
  )

  const updateQuickConnectRecents = useCallback(
    (update: (recents: RecentQuickConnection[]) => RecentQuickConnection[]) => {
      setQuickConnectRecents((current) => {
        const next = update(current)
        saveRecentQuickConnections(window.localStorage, next)
        return next
      })
    },
    []
  )

  const handleForgetQuickConnectRecent = useCallback(
    (recent: RecentQuickConnection) =>
      updateQuickConnectRecents((current) => removeRecentQuickConnection(current, recent)),
    [updateQuickConnectRecents]
  )

  const handleClearQuickConnectRecents = useCallback(
    () => updateQuickConnectRecents(() => []),
    [updateQuickConnectRecents]
  )

  const openSaveQuickConnectDialog = useCallback(
    (tab: Tab) => {
      setEditingProfile(null)
      setDuplicatingProfile(null)
      setDraftProfile(draftProfileFromTab(tab))
      setSavingQuickConnectTabId(tab.id)
      setShowConnectionDialog(true)
    },
    [
      setDraftProfile,
      setDuplicatingProfile,
      setEditingProfile,
      setSavingQuickConnectTabId,
      setShowConnectionDialog,
    ]
  )

  const handleQuickConnectSaved = useCallback(
    (profile: SavedProfile) => {
      const tabId = savingQuickConnectTabId
      if (!tabId) return
      // The tab now belongs to the profile, so its SFTP and reconnects use what was saved.
      const profileConnection = buildConnectionFromProfile(profile).connection
      updateTab(tabId, (tab) => {
        // A tab still signing in automatically keeps doing so: it reuses the
        // password typed in it, whether or not the profile stored it.
        const keepAuto = tab.connection?.authMethod === "auto"
        return {
          ...tab,
          title: profile.name,
          quickConnect: false,
          quickConnectAuth: undefined,
          connection: {
            ...tab.connection,
            ...profileConnection,
            ...(keepAuto && {
              authMethod: "auto" as const,
              privateKeyPath: tab.connection?.privateKeyPath,
              jumpHosts: tab.connection?.jumpHosts,
            }),
          },
        }
      })
    },
    [savingQuickConnectTabId, updateTab]
  )

  const handleTerminalConnectionProgress = useCallback(
    (tabId: string, progress: SshConnectionProgress) => {
      const auth = progress.auth
      if (progress.phase === "target_authenticated" && auth) {
        updateTab(tabId, (tab) => applyQuickConnectAuth(tab, auth))
        return
      }

      if (progress.phase !== "ready") return
      const tab = tabsRef.current.find((candidate) => candidate.id === tabId)
      if (!tab?.quickConnect || tab.connection?.profileId) return
      // Only the first connection is offered for saving; reconnects and restored sessions are not.
      updateTab(tabId, (current) => ({ ...current, quickConnect: false }))

      const connection = quickConnectionFromTab(tab)
      if (connection) {
        updateQuickConnectRecents((current) =>
          addRecentQuickConnection(current, connection, Date.now())
        )
      }
      toast({
        title: t("quickConnect.connectedTitle", { target: tab.title }),
        description: t("quickConnect.connectedDescription"),
        duration: 10_000,
        action: (
          <ToastAction
            altText={t("quickConnect.saveAsProfile")}
            onClick={() => {
              const current = tabsRef.current.find((candidate) => candidate.id === tabId)
              if (current) openSaveQuickConnectDialog(current)
            }}
          >
            {t("quickConnect.saveAsProfile")}
          </ToastAction>
        ),
      })
    },
    [openSaveQuickConnectDialog, t, tabsRef, updateQuickConnectRecents, updateTab]
  )

  return {
    quickConnectRecents,
    handleOpenConnectionDialogFromLauncher,
    handleForgetQuickConnectRecent,
    handleClearQuickConnectRecents,
    openSaveQuickConnectDialog,
    handleQuickConnectSaved,
    handleTerminalConnectionProgress,
  }
}
