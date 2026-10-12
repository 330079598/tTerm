import { useCallback } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { BroadcastRuntime } from "@/components/TTermApp/useBroadcastRuntime"
import type { useTransferManager } from "@/contexts/TransferContext"
import { getAdjacentTabId, getTabIdsForCloseAction } from "@/lib/tabClosing"
import { isPageTab, Tab } from "@/types/tab"

interface TabClosingDeps {
  transfers: ReturnType<typeof useTransferManager>["transfers"]
  stopLiveSource: BroadcastRuntime["stopLiveSource"]
}

/**
 * Closing tabs, after confirming when that would discard unsaved remote
 * files or hide running transfers.
 */
export function useTabClosing(shell: AppShell, deps: TabClosingDeps) {
  const {
    t,
    tabsRef,
    activeTabIdRef,
    workspaceRef,
    confirm,
    cleanupConnection,
    tabs,
    activeTabId,
    removeTabs,
  } = shell
  const { transfers, stopLiveSource } = deps

  const getActiveTransfersForTabs = useCallback(
    (tabIds: string[]) => {
      const idSet = new Set(tabIds)
      return transfers.filter(
        (transfer) =>
          transfer.tabId &&
          idSet.has(transfer.tabId) &&
          (transfer.status === "pending" || transfer.status === "transferring")
      )
    },
    [transfers]
  )

  const confirmCloseTabsWithTransfers = useCallback(
    async (tabIds: string[]) => {
      const activeTransfers = getActiveTransfersForTabs(tabIds)
      if (activeTransfers.length === 0) {
        return true
      }

      return confirm({
        title: t("tabs.closeActiveTransferTitle", {
          count: activeTransfers.length,
          defaultValue: "Active transfer in progress",
        }),
        description: t("tabs.closeActiveTransferDescription", {
          count: activeTransfers.length,
          defaultValue:
            "Closing this tab will hide active SFTP transfer progress and may interrupt the transfer. Continue?",
        }),
        confirmText: t("tabs.closeActiveTransferConfirm", { defaultValue: "Close anyway" }),
        cancelText: t("common.cancel", { defaultValue: "Cancel" }),
        variant: "destructive",
      })
    },
    [confirm, getActiveTransfersForTabs, t]
  )

  const confirmCloseTabsWithUnsavedRemoteFiles = useCallback(
    async (tabIds: string[]) => {
      const idSet = new Set(tabIds)
      const unsavedTabs = tabsRef.current.filter(
        (tab) => idSet.has(tab.id) && tab.type === "remote-file-editor" && tab.isModified
      )
      if (unsavedTabs.length === 0) {
        return true
      }

      return confirm({
        title: t("remoteFileEditor.closeUnsavedTitle", {
          count: unsavedTabs.length,
          defaultValue: "Close unsaved remote file?",
        }),
        description: t("remoteFileEditor.closeUnsavedDescription", {
          count: unsavedTabs.length,
          defaultValue:
            "One or more remote file tabs have unsaved changes. Closing them will discard those changes.",
        }),
        confirmText: t("remoteFileEditor.closeUnsavedConfirm", { defaultValue: "Discard" }),
        cancelText: t("common.cancel", { defaultValue: "Cancel" }),
        variant: "destructive",
      })
    },
    [confirm, t, tabsRef]
  )

  const closeTabById = useCallback(
    (id: string, preferredActiveTabId?: string) => {
      const tab = tabsRef.current.find((currentTab) => currentTab.id === id)
      if (tab) {
        stopLiveSource(tab.id, tab.sessionNonce ?? 0)
      }
      if (!isPageTab(tab) && tab?.type !== "remote-file-editor") {
        cleanupConnection(id)
      }
      if (activeTabIdRef.current === id && preferredActiveTabId) {
        workspaceRef.current?.activateTab(preferredActiveTabId)
      }
      removeTabs([id], { preferredActiveTabId })
    },
    [activeTabIdRef, cleanupConnection, removeTabs, stopLiveSource, tabsRef, workspaceRef]
  )

  const handleRemoveTab = useCallback(
    async (id: string) => {
      const tabIds =
        workspaceRef.current?.getGroupTabIds(id) ??
        tabsRef.current.map((currentTab) => currentTab.id)
      const preferredActiveTabId = getAdjacentTabId(tabIds, id)
      const unsavedConfirmed = await confirmCloseTabsWithUnsavedRemoteFiles([id])
      if (!unsavedConfirmed) {
        return
      }

      const confirmed = await confirmCloseTabsWithTransfers([id])
      if (!confirmed) {
        return
      }
      closeTabById(id, preferredActiveTabId)
    },
    [
      closeTabById,
      confirmCloseTabsWithTransfers,
      confirmCloseTabsWithUnsavedRemoteFiles,
      tabsRef,
      workspaceRef,
    ]
  )

  const closeTabsWithConfirmation = useCallback(
    async (targetTabs: Tab[], closeAction: () => void) => {
      const targetTabIds = targetTabs.map((tab) => tab.id)
      const unsavedConfirmed = await confirmCloseTabsWithUnsavedRemoteFiles(targetTabIds)
      if (!unsavedConfirmed) {
        return
      }

      const confirmed = await confirmCloseTabsWithTransfers(targetTabIds)
      if (!confirmed) {
        return
      }

      for (const targetTab of targetTabs) {
        stopLiveSource(targetTab.id, targetTab.sessionNonce ?? 0)
        if (!isPageTab(targetTab) && targetTab.type !== "remote-file-editor") {
          cleanupConnection(targetTab.id)
        }
      }
      closeAction()
    },
    [
      cleanupConnection,
      confirmCloseTabsWithTransfers,
      confirmCloseTabsWithUnsavedRemoteFiles,
      stopLiveSource,
    ]
  )

  const handleCloseOtherTabs = useCallback(
    async (id: string) => {
      const tabIds = workspaceRef.current?.getGroupTabIds(id) ?? tabs.map((tab) => tab.id)
      const targetIds = getTabIdsForCloseAction(tabIds, id, "close-others")
      const targetIdSet = new Set(targetIds)
      const targetTabs = tabs.filter((tab) => targetIdSet.has(tab.id))
      await closeTabsWithConfirmation(targetTabs, () => {
        workspaceRef.current?.activateTab(id)
        removeTabs(targetIds, { preferredActiveTabId: id, activatePreferred: true })
      })
    },
    [closeTabsWithConfirmation, removeTabs, tabs, workspaceRef]
  )

  const handleCloseTabsToLeft = useCallback(
    async (id: string) => {
      const tabIds = workspaceRef.current?.getGroupTabIds(id) ?? tabs.map((tab) => tab.id)
      const targetIds = getTabIdsForCloseAction(tabIds, id, "close-left")
      if (targetIds.length === 0) {
        return
      }

      const targetIdSet = new Set(targetIds)
      const targetTabs = tabs.filter((tab) => targetIdSet.has(tab.id))
      await closeTabsWithConfirmation(targetTabs, () => {
        if (activeTabId && targetIdSet.has(activeTabId)) {
          workspaceRef.current?.activateTab(id)
        }
        removeTabs(targetIds, { preferredActiveTabId: id })
      })
    },
    [activeTabId, closeTabsWithConfirmation, removeTabs, tabs, workspaceRef]
  )

  const handleCloseTabsToRight = useCallback(
    async (id: string) => {
      const tabIds = workspaceRef.current?.getGroupTabIds(id) ?? tabs.map((tab) => tab.id)
      const targetIds = getTabIdsForCloseAction(tabIds, id, "close-right")
      if (targetIds.length === 0) {
        return
      }

      const targetIdSet = new Set(targetIds)
      const targetTabs = tabs.filter((tab) => targetIdSet.has(tab.id))
      await closeTabsWithConfirmation(targetTabs, () => {
        if (activeTabId && targetIdSet.has(activeTabId)) {
          workspaceRef.current?.activateTab(id)
        }
        removeTabs(targetIds, { preferredActiveTabId: id })
      })
    },
    [activeTabId, closeTabsWithConfirmation, removeTabs, tabs, workspaceRef]
  )

  return {
    handleRemoveTab,
    handleCloseOtherTabs,
    handleCloseTabsToLeft,
    handleCloseTabsToRight,
  }
}
