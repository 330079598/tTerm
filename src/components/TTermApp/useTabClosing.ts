import { useCallback } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { BroadcastRuntime } from "@/components/TTermApp/useBroadcastRuntime"
import type { useTransferManager } from "@/contexts/TransferContext"
import { getTabIdsForCloseAction, type TabCloseAction } from "@/lib/tabClosing"
import { findGroupOfTab, getPaneIds } from "@/lib/tabGroups"
import { isPageTab, isSessionTab, Tab } from "@/types/tab"

interface TabClosingDeps {
  transfers: ReturnType<typeof useTransferManager>["transfers"]
  stopLiveSource: BroadcastRuntime["stopLiveSource"]
}

/**
 * Closing panes and the tabs of the bar that hold them, after confirming
 * when that would close several sessions at once, discard unsaved remote
 * files or hide running transfers.
 */
export function useTabClosing(shell: AppShell, deps: TabClosingDeps) {
  const { t, tabsRef, confirm, cleanupConnection, tabs, groups, removeTabs } = shell
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
    (id: string) => {
      const tab = tabsRef.current.find((currentTab) => currentTab.id === id)
      if (tab) {
        stopLiveSource(tab.id, tab.sessionNonce ?? 0)
      }
      if (!isPageTab(tab) && tab?.type !== "remote-file-editor") {
        cleanupConnection(id)
      }
      removeTabs([id])
    },
    [cleanupConnection, removeTabs, stopLiveSource, tabsRef]
  )

  /** Closes one pane; the next pane of its tab takes the focus, or the next tab once it was the last. */
  const handleRemoveTab = useCallback(
    async (id: string) => {
      const unsavedConfirmed = await confirmCloseTabsWithUnsavedRemoteFiles([id])
      if (!unsavedConfirmed) {
        return
      }

      const confirmed = await confirmCloseTabsWithTransfers([id])
      if (!confirmed) {
        return
      }
      closeTabById(id)
    },
    [closeTabById, confirmCloseTabsWithTransfers, confirmCloseTabsWithUnsavedRemoteFiles]
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

  // Closing a tab split into several sessions takes them all, so it asks first.
  const confirmCloseSplitTab = useCallback(
    async (targetTabs: Tab[]) => {
      const sessions = targetTabs.filter((tab) => isSessionTab(tab)).length
      if (sessions < 2) {
        return true
      }
      return confirm({
        title: t("workspace.closeSplitTabTitle", { count: targetTabs.length }),
        description: t("workspace.closeSplitTabDescription", { count: targetTabs.length }),
        confirmText: t("workspace.closeSplitTabConfirm"),
        cancelText: t("common.cancel", { defaultValue: "Cancel" }),
        variant: "destructive",
      })
    },
    [confirm, t]
  )

  /** Closes a tab of the bar with all of its panes. */
  const handleCloseTab = useCallback(
    async (id: string) => {
      const group = findGroupOfTab(groups, id)
      const paneIds = new Set(group ? getPaneIds(group.root) : [id])
      const targetTabs = tabs.filter((tab) => paneIds.has(tab.id))
      if (!(await confirmCloseSplitTab(targetTabs))) {
        return
      }
      await closeTabsWithConfirmation(targetTabs, () => removeTabs([...paneIds]))
    },
    [closeTabsWithConfirmation, confirmCloseSplitTab, groups, removeTabs, tabs]
  )

  // The other tabs of the bar, or those on one side of the one holding `id`,
  // with every pane they hold.
  const closeTabsOfBar = useCallback(
    async (id: string, action: TabCloseAction) => {
      const group = findGroupOfTab(groups, id)
      if (!group) {
        return
      }
      const targetGroupIds = new Set(
        getTabIdsForCloseAction(
          groups.map((candidate) => candidate.id),
          group.id,
          action
        )
      )
      const targetIds = groups
        .filter((candidate) => targetGroupIds.has(candidate.id))
        .flatMap((candidate) => getPaneIds(candidate.root))
      if (targetIds.length === 0) {
        return
      }
      const targetIdSet = new Set(targetIds)
      const targetTabs = tabs.filter((tab) => targetIdSet.has(tab.id))
      await closeTabsWithConfirmation(targetTabs, () =>
        removeTabs(targetIds, {
          preferredActiveTabId: group.activeTabId,
          activatePreferred: action === "close-others",
        })
      )
    },
    [closeTabsWithConfirmation, groups, removeTabs, tabs]
  )

  const handleCloseOtherTabs = useCallback(
    (id: string) => closeTabsOfBar(id, "close-others"),
    [closeTabsOfBar]
  )
  const handleCloseTabsToLeft = useCallback(
    (id: string) => closeTabsOfBar(id, "close-left"),
    [closeTabsOfBar]
  )
  const handleCloseTabsToRight = useCallback(
    (id: string) => closeTabsOfBar(id, "close-right"),
    [closeTabsOfBar]
  )

  return {
    handleRemoveTab,
    handleCloseTab,
    handleCloseOtherTabs,
    handleCloseTabsToLeft,
    handleCloseTabsToRight,
  }
}
