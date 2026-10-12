import { invoke } from "@tauri-apps/api/core"
import { useCallback } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { ProfileDialogState } from "@/components/TTermApp/useProfileDialogState"
import type { BroadcastState } from "@/components/TTermApp/useBroadcastState"
import type { BroadcastRuntime } from "@/components/TTermApp/useBroadcastRuntime"
import type { Tab } from "@/types/tab"
import type { ProfileSavedOptions } from "@/components/ConnectionDialog/types"
import { SavedProfile } from "@/components/ProfilesPanel"
import { ToastAction } from "@/components/ui/toast"
import { toast } from "@/hooks/use-toast"
import { applyProfileToTab } from "@/lib/profileConnections"

interface ProfileActionDeps {
  liveSourceRef: BroadcastState["liveSourceRef"]
  runtimeStatesRef: BroadcastState["runtimeStatesRef"]
  stopLiveSource: BroadcastRuntime["stopLiveSource"]
  openSaveQuickConnectDialog: (tab: Tab) => void
  handleQuickConnectSaved: (profile: SavedProfile) => void
  handleReconnectTab: (tabId: string) => void
}

/**
 * Editing, duplicating and deleting profiles, and updating the open tabs
 * of a profile once it is saved.
 */
export function useProfileActions(
  shell: AppShell,
  dialog: ProfileDialogState,
  deps: ProfileActionDeps
) {
  const { t, tabsRef, confirm, cleanupConnection, tabs, removeTabs, updateTab } = shell
  const {
    setShowConnectionDialog,
    savingQuickConnectTabId,
    setEditingProfile,
    editingProfileTabId,
    setEditingProfileTabId,
    setDuplicatingProfile,
  } = dialog
  const {
    liveSourceRef,
    runtimeStatesRef,
    stopLiveSource,
    openSaveQuickConnectDialog,
    handleQuickConnectSaved,
    handleReconnectTab,
  } = deps

  const handleEditProfile = useCallback(
    (profile: SavedProfile) => {
      setDuplicatingProfile(null)
      setEditingProfile(profile)
      setEditingProfileTabId(null)
      setShowConnectionDialog(true)
    },
    [setDuplicatingProfile, setEditingProfile, setEditingProfileTabId, setShowConnectionDialog]
  )

  const handleDuplicateProfile = useCallback(
    (profile: SavedProfile) => {
      setEditingProfile(null)
      setDuplicatingProfile(profile)
      setShowConnectionDialog(true)
    },
    [setDuplicatingProfile, setEditingProfile, setShowConnectionDialog]
  )

  const handleDeleteProfile = useCallback(
    (profile: SavedProfile) => {
      const profileTabs = tabs.filter(
        (tab) => tab.type === "ssh" && tab.connection?.profileId === profile.id
      )

      const source = liveSourceRef.current
      if (source && profileTabs.some((tab) => tab.id === source.tabId)) {
        stopLiveSource(source.tabId, source.sessionNonce)
      }

      for (const tab of profileTabs) {
        cleanupConnection(tab.id)
      }
      removeTabs(profileTabs.map((tab) => tab.id))
    },
    [cleanupConnection, liveSourceRef, removeTabs, stopLiveSource, tabs]
  )

  const handleEditTabProfile = useCallback(
    async (tab: Tab) => {
      const profileId = tab.connection?.profileId
      if (!profileId) {
        if (tab.type === "ssh") openSaveQuickConnectDialog(tab)
        return
      }

      try {
        const profiles = await invoke<SavedProfile[]>("list_profiles")
        const profile = profiles.find((item) => item.id === profileId)
        if (!profile) {
          toast({
            title: t("common.error", { defaultValue: "Error" }),
            description: t("profiles.notFound", {
              defaultValue: "This connection configuration no longer exists.",
            }),
            variant: "destructive",
          })
          return
        }

        handleEditProfile(profile)
        setEditingProfileTabId(tab.id)
      } catch (error) {
        console.error("Failed to load connection profile:", error)
        toast({
          title: t("common.error", { defaultValue: "Error" }),
          description: t("profiles.loadFailed", {
            defaultValue: "Failed to load connection profiles.",
          }),
          variant: "destructive",
        })
      }
    },
    [handleEditProfile, openSaveQuickConnectDialog, setEditingProfileTabId, t]
  )

  const handleProfileSaved = useCallback(
    (profile: SavedProfile, { reconnect }: ProfileSavedOptions) => {
      if (savingQuickConnectTabId) {
        handleQuickConnectSaved(profile)
        return
      }

      // Open tabs of the profile reconnect with what was saved; their sessions keep running.
      const profileTabs = tabsRef.current.filter(
        (tab) => tab.type === "ssh" && tab.connection?.profileId === profile.id
      )
      profileTabs.forEach((tab) =>
        updateTab(tab.id, (current) => applyProfileToTab(current, profile))
      )

      const reconnectTabId =
        reconnect && profileTabs.some((tab) => tab.id === editingProfileTabId)
          ? editingProfileTabId
          : null
      if (reconnectTabId) handleReconnectTab(reconnectTabId)

      const isLive = (tab: Tab) => {
        const runtime = runtimeStatesRef.current[tab.id]
        return (
          runtime?.sessionNonce === (tab.sessionNonce ?? 0) &&
          (runtime.connectionState === "connected" ||
            runtime.connectionState === "connecting" ||
            runtime.connectionState === "reconnecting")
        )
      }
      const liveTabIds = profileTabs
        .filter((tab) => tab.id !== reconnectTabId && isLive(tab))
        .map((tab) => tab.id)
      if (liveTabIds.length === 0) return

      toast({
        title: t("profiles.openTabsNotUpdatedTitle"),
        description: t("profiles.openTabsNotUpdatedDescription", { count: liveTabIds.length }),
        duration: 10_000,
        action: (
          <ToastAction
            altText={t("profiles.reconnectNow")}
            onClick={async () => {
              const targets = tabsRef.current.filter(
                (tab) =>
                  liveTabIds.includes(tab.id) &&
                  tab.connection?.profileId === profile.id &&
                  isLive(tab)
              )
              if (targets.length === 0) return
              const confirmed = await confirm({
                title: t("profiles.reconnectConfirmTitle", { count: targets.length }),
                description: t("profiles.reconnectConfirmDescription"),
                confirmText: t("profiles.reconnectConfirmAction"),
                variant: "destructive",
              })
              if (confirmed) targets.forEach((tab) => handleReconnectTab(tab.id))
            }}
          >
            {t("profiles.reconnectNow")}
          </ToastAction>
        ),
      })
    },
    [
      confirm,
      editingProfileTabId,
      handleQuickConnectSaved,
      handleReconnectTab,
      runtimeStatesRef,
      savingQuickConnectTabId,
      t,
      tabsRef,
      updateTab,
    ]
  )

  return {
    handleEditProfile,
    handleDuplicateProfile,
    handleDeleteProfile,
    handleEditTabProfile,
    handleProfileSaved,
  }
}
