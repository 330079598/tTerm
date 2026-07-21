import "@/components/TTermApp.css"
import { invoke } from "@tauri-apps/api/core"
import { platform } from "@tauri-apps/plugin-os"
import type { SerializedDockview } from "dockview-react"
import { BookMarked, Minus, Plus, Settings, Square, X } from "lucide-react"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { ConnectionDialog } from "@/components/ConnectionDialog"
import { ContextMenu } from "@/components/ContextMenu"
import { ProfilesPanel, SavedProfile } from "@/components/ProfilesPanel"
import { RenameDialog } from "@/components/RenameDialog"
import { TabBar } from "@/components/TabBar"
import { TransferManager } from "@/components/TransferManager"
import { EmptyState } from "@/components/TTermApp/EmptyState"
import {
  TabPanels,
  type TabPanelsHandle,
  type WorkspaceSplitDirection,
} from "@/components/TTermApp/TabPanels"
import { buildTabFromConnection } from "@/components/TTermApp/ttermAppUtils"
import { VaultStartupUnlockDialog } from "@/components/VaultStartupUnlockDialog"
import { formatBytes, MAX_EDIT_FILE_BYTES } from "@/components/SftpDrawer/sftpDrawerUtils"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import { useConfig } from "@/contexts/ConfigContext"
import { useTransferManager } from "@/contexts/TransferContext"
import { useConnectionManager } from "@/hooks/useConnectionManager"
import { useSessionPersistence } from "@/hooks/useSessionPersistence"
import { useTabContextMenu } from "@/hooks/useTabContextMenu"
import { useTabs } from "@/hooks/useTabs"
import { toast } from "@/hooks/use-toast"
import { useWindowControls } from "@/hooks/useWindowControls"
import { markSessionReady } from "@/lib/startup"
import { Tab } from "@/types/tab"

const SETTINGS_TAB_TITLE = "Settings"

function formatRemoteFileConnectionLabel(connection?: Tab["connection"]): string | undefined {
  const profileName = connection?.profileName?.trim()
  if (profileName) {
    return profileName
  }

  const host = connection?.host?.trim()
  if (!host) {
    return undefined
  }

  const userHost = connection?.username ? `${connection.username}@${host}` : host
  return connection?.port && connection.port !== 22 ? `${userHost}:${connection.port}` : userHost
}

function getRemoteFileConnectionKey(connection?: Tab["connection"]): string {
  if (connection?.profileId) {
    return `profile:${connection.profileId}`
  }

  const host = connection?.host?.trim() ?? ""
  const username = connection?.username?.trim() ?? ""
  const port = connection?.port ?? 22
  return `target:${username}@${host}:${port}`
}

export const TTermApp: React.FC = () => {
  const { t, i18n } = useTranslation()
  const [os] = useState<string>(() => platform())
  const isMacos = os === "macos"
  const isLinux = os === "linux"
  const isWindows = os === "windows"
  const [showConnectionDialog, setShowConnectionDialog] = useState(false)
  const [showProfilesPanel, setShowProfilesPanel] = useState(false)
  const [editingProfile, setEditingProfile] = useState<SavedProfile | null>(null)
  const [duplicatingProfile, setDuplicatingProfile] = useState<SavedProfile | null>(null)
  const [profilesRefreshKey, setProfilesRefreshKey] = useState(0)
  const [sessionRestored, setSessionRestored] = useState(false)
  const [workspaceLayout, setWorkspaceLayout] = useState<SerializedDockview | null>(null)
  const [startupVaultUnlockDismissed, setStartupVaultUnlockDismissed] = useState(false)
  const workspaceRef = useRef<TabPanelsHandle>(null)

  const {
    tabs,
    activeTabId,
    addTab,
    openSettingsTab,
    renameSettingsTab,
    removeTab,
    removeTabs,
    setActiveTab,
    moveTab,
    duplicateTab,
    closeOtherTabs,
    closeTabsToRight,
    closeTabsToLeft,
    renameTab,
    restoreSession,
    updateTab,
  } = useTabs()

  const { saveSession, loadSession } = useSessionPersistence()
  const { cleanupConnection } = useConnectionManager()
  const { config, isLoaded, saveConfig, secretStatus } = useConfig()
  const { cancelTransfer, clearCompletedTransfers, removeTransfer, transfers } =
    useTransferManager()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const settingsTabTitle = t("settings.title", { defaultValue: SETTINGS_TAB_TITLE })
  const shouldPromptStartupVaultUnlock =
    isLoaded &&
    config.secret_vault_enabled &&
    (config.secret_storage_mode === "vault" ||
      config.secret_storage_mode === "auto" ||
      config.secret_storage_mode === "hybrid") &&
    (config.prompt_unlock_vault_on_startup || config.secret_storage_mode === "hybrid") &&
    !secretStatus.vaultUnlocked &&
    !startupVaultUnlockDismissed
  const startupConnectionsReady = !shouldPromptStartupVaultUnlock

  useEffect(() => {
    if (isLoaded) {
      i18n.changeLanguage(config.language)
    }
  }, [isLoaded, config.language, i18n])

  useEffect(() => {
    renameSettingsTab(settingsTabTitle)
  }, [renameSettingsTab, settingsTabTitle])

  useEffect(() => {
    if (!isLoaded) {
      return
    }

    let cancelled = false

    const loadAndRestoreSession = async () => {
      try {
        const savedSession = await loadSession()
        if (cancelled) {
          return
        }

        if (savedSession && savedSession.tabs.length > 0) {
          setWorkspaceLayout(savedSession.layout)
          restoreSession(savedSession.tabs, savedSession.activeTabId)
        } else {
          addTab(
            buildTabFromConnection({
              title: "Terminal",
              type: "terminal",
              isModified: false,
            })
          )
        }
      } finally {
        if (!cancelled) {
          setSessionRestored(true)
          markSessionReady()
        }
      }
    }

    void loadAndRestoreSession()

    return () => {
      cancelled = true
    }
  }, [addTab, isLoaded, loadSession, restoreSession])

  useEffect(() => {
    if (!sessionRestored) {
      return
    }

    saveSession(tabs, activeTabId, workspaceLayout)
  }, [tabs, activeTabId, saveSession, sessionRestored, workspaceLayout])

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

  const handleNewTab = useCallback(() => {
    setShowConnectionDialog(true)
  }, [])

  const handleConnect = useCallback(
    (connection: Omit<Tab, "id" | "isActive">) => {
      addTab(buildTabFromConnection(connection))
    },
    [addTab]
  )

  const handleEditProfile = useCallback((profile: SavedProfile) => {
    setDuplicatingProfile(null)
    setEditingProfile(profile)
    setShowConnectionDialog(true)
  }, [])

  const handleDuplicateProfile = useCallback((profile: SavedProfile) => {
    setEditingProfile(null)
    setDuplicatingProfile(profile)
    setShowConnectionDialog(true)
  }, [])

  const handleDeleteProfile = useCallback(
    (profile: SavedProfile) => {
      const profileTabs = tabs.filter(
        (tab) => tab.type === "ssh" && tab.connection?.profileId === profile.id
      )

      for (const tab of profileTabs) {
        cleanupConnection(tab.id)
      }
      removeTabs(profileTabs.map((tab) => tab.id))
    },
    [cleanupConnection, removeTabs, tabs]
  )

  const handleEditTabProfile = useCallback(
    async (tab: Tab) => {
      const profileId = tab.connection?.profileId
      if (!profileId) {
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
    [handleEditProfile, t]
  )

  const handleOpenRemoteFile = useCallback(
    (entry: SftpDirectoryEntry, sourceTabId: string, connection?: Tab["connection"]) => {
      const fileSize = entry.size ?? 0
      if (fileSize > MAX_EDIT_FILE_BYTES) {
        const message = `Remote file is too large to edit in tTerm (${fileSize} bytes, limit ${MAX_EDIT_FILE_BYTES} bytes)`
        toast({
          variant: "destructive",
          title: t("remoteFileEditor.openFailed", { defaultValue: "Failed to open remote file" }),
          description: `${message} (${formatBytes(fileSize)}, limit ${formatBytes(MAX_EDIT_FILE_BYTES)})`,
        })
        return
      }

      const host = connection?.host
      const connectionLabel = formatRemoteFileConnectionLabel(connection)
      const connectionKey = getRemoteFileConnectionKey(connection)
      const existingTab = tabs.find(
        (tab) =>
          tab.type === "remote-file-editor" &&
          tab.remoteFile?.path === entry.path &&
          (tab.remoteFile.connectionKey ?? getRemoteFileConnectionKey(tab.connection)) ===
            connectionKey
      )

      if (existingTab) {
        setActiveTab(existingTab.id)
        return
      }

      addTab({
        title: entry.name,
        type: "remote-file-editor",
        isModified: false,
        connection,
        remoteFile: {
          sourceTabId,
          profileId: connection?.profileId,
          profileName: connection?.profileName,
          connectionLabel,
          connectionKey,
          host,
          path: entry.path,
          fileName: entry.name,
          size: entry.size ?? 0,
          modifiedAt: entry.modifiedAt,
        },
      })
    },
    [addTab, setActiveTab, t, tabs]
  )

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
      const unsavedTabs = tabs.filter(
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
    [confirm, t, tabs]
  )

  const closeTabById = useCallback(
    (id: string) => {
      const tab = tabs.find((currentTab) => currentTab.id === id)
      if (tab?.type !== "settings" && tab?.type !== "remote-file-editor") {
        cleanupConnection(id)
      }
      removeTab(id)
    },
    [cleanupConnection, removeTab, tabs]
  )

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
        if (targetTab.type !== "settings" && targetTab.type !== "remote-file-editor") {
          cleanupConnection(targetTab.id)
        }
      }
      closeAction()
    },
    [cleanupConnection, confirmCloseTabsWithTransfers, confirmCloseTabsWithUnsavedRemoteFiles]
  )

  const handleCloseOtherTabs = useCallback(
    async (id: string) => {
      const groupTabIds = workspaceRef.current?.getGroupTabIds(id)
      const targetIdSet = groupTabIds ? new Set(groupTabIds.filter((tabId) => tabId !== id)) : null
      const targetTabs = targetIdSet
        ? tabs.filter((tab) => targetIdSet.has(tab.id))
        : tabs.filter((tab) => tab.id !== id)
      await closeTabsWithConfirmation(targetTabs, () =>
        targetIdSet ? removeTabs([...targetIdSet]) : closeOtherTabs(id)
      )
    },
    [closeOtherTabs, closeTabsWithConfirmation, removeTabs, tabs]
  )

  const handleCloseTabsToLeft = useCallback(
    async (id: string) => {
      const groupTabIds = workspaceRef.current?.getGroupTabIds(id)
      const tabIndex = groupTabIds
        ? groupTabIds.indexOf(id)
        : tabs.findIndex((tab) => tab.id === id)
      const targetIds = groupTabIds?.slice(0, tabIndex)
      if (tabIndex <= 0 || (targetIds && targetIds.length === 0)) {
        return
      }

      const targetIdSet = targetIds ? new Set(targetIds) : null
      const targetTabs = targetIdSet
        ? tabs.filter((tab) => targetIdSet.has(tab.id))
        : tabs.slice(0, tabIndex)
      await closeTabsWithConfirmation(targetTabs, () =>
        targetIds ? removeTabs(targetIds) : closeTabsToLeft(id)
      )
    },
    [closeTabsToLeft, closeTabsWithConfirmation, removeTabs, tabs]
  )

  const handleCloseTabsToRight = useCallback(
    async (id: string) => {
      const groupTabIds = workspaceRef.current?.getGroupTabIds(id)
      const tabIndex = groupTabIds
        ? groupTabIds.indexOf(id)
        : tabs.findIndex((tab) => tab.id === id)
      if (tabIndex === -1) {
        return
      }

      const targetIds = groupTabIds?.slice(tabIndex + 1)
      const targetIdSet = targetIds ? new Set(targetIds) : null
      const targetTabs = targetIdSet
        ? tabs.filter((tab) => targetIdSet.has(tab.id))
        : tabs.slice(tabIndex + 1)
      await closeTabsWithConfirmation(targetTabs, () =>
        targetIds ? removeTabs(targetIds) : closeTabsToRight(id)
      )
    },
    [closeTabsToRight, closeTabsWithConfirmation, removeTabs, tabs]
  )

  const handleSplitTab = useCallback((tabId: string, direction: WorkspaceSplitDirection) => {
    workspaceRef.current?.splitTab(tabId, direction)
  }, [])

  const handleTabDragMove = useCallback((tabId: string, clientX: number, clientY: number) => {
    workspaceRef.current?.previewTabDrop(tabId, clientX, clientY)
  }, [])

  const handleTabDrop = useCallback((tabId: string, clientX: number, clientY: number) => {
    return workspaceRef.current?.commitTabDrop(tabId, clientX, clientY) ?? false
  }, [])

  const handleTabDragCancel = useCallback(() => {
    workspaceRef.current?.clearTabDrop()
  }, [])

  useEffect(() => {
    const handleWorkspaceShortcut = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.key !== "\\") {
        return
      }
      if (!activeTabId) {
        return
      }

      event.preventDefault()
      handleSplitTab(activeTabId, event.shiftKey ? "below" : "right")
    }

    window.addEventListener("keydown", handleWorkspaceShortcut, true)
    return () => window.removeEventListener("keydown", handleWorkspaceShortcut, true)
  }, [activeTabId, handleSplitTab])

  const {
    nativeControlsReservePx,
    handleMinimizeWindow,
    handleToggleMaximizeWindow,
    handleCloseWindow,
  } = useWindowControls(isWindows)

  const {
    contextMenu,
    renameDialogState,
    handleTabContextMenu,
    handleContextMenuAction,
    handleCloseContextMenu,
    handleRenameConfirm,
    handleRenameClose,
  } = useTabContextMenu({
    handleNewTab,
    duplicateTab,
    splitTab: handleSplitTab,
    handleRemoveTab,
    handleCloseOtherTabs,
    handleCloseTabsToRight,
    handleCloseTabsToLeft,
    updateTab,
    renameTab,
    editTabProfile: handleEditTabProfile,
  })

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

  const handleSettingsClick = useCallback(() => {
    openSettingsTab(settingsTabTitle)
  }, [openSettingsTab, settingsTabTitle])

  const handleCollapsedProfileGroupKeysChange = useCallback(
    (groups: string[]) => {
      saveConfig({ collapsed_profile_group_keys: groups }).catch((error) => {
        console.error("Failed to save collapsed profile groups:", error)
      })
    },
    [saveConfig]
  )

  const renderTabContent = () => {
    if (tabs.length === 0) {
      return (
        <EmptyState
          collapsedProfileGroupKeys={config.collapsed_profile_group_keys}
          handleConnect={handleConnect}
          handleNewTab={handleNewTab}
          onCollapsedProfileGroupKeysChange={handleCollapsedProfileGroupKeysChange}
          onDuplicateProfile={handleDuplicateProfile}
          onDeleteProfile={handleDeleteProfile}
          onEditProfile={handleEditProfile}
          refreshKey={profilesRefreshKey}
        />
      )
    }

    return (
      <TabPanels
        ref={workspaceRef}
        activeTabId={activeTabId}
        duplicateTab={duplicateTab}
        handlePinConnectionHeader={handlePinConnectionHeader}
        handleReconnectTab={handleReconnectTab}
        handleServerMonitorVisibilityChange={handleServerMonitorVisibilityChange}
        handleUnpinConnectionHeader={handleUnpinConnectionHeader}
        initialLayout={workspaceLayout}
        onActiveTabChange={setActiveTab}
        onConnectProfile={handleConnect}
        onEditProfile={handleEditProfile}
        onLayoutChange={setWorkspaceLayout}
        onOpenRemoteFile={handleOpenRemoteFile}
        onTabClose={handleRemoveTab}
        onTabContextMenu={handleTabContextMenu}
        profilesRefreshKey={profilesRefreshKey}
        startupConnectionsReady={startupConnectionsReady}
        startupSessionRestoreMode={config.startup_session_restore_mode}
        tabs={tabs}
        updateTab={updateTab}
      />
    )
  }

  return (
    <div
      className={`app ${isMacos ? "macos" : ""} ${isLinux ? "linux" : ""}`}
      data-tab-width-mode={config.tab_width_mode}
      style={
        {
          "--tab-standard-width": `${config.tab_standard_width}px`,
        } as React.CSSProperties
      }
    >
      <div className="title-bar">
        <div className="title-bar-left">
          <div className="tab-list-container">
            <TabBar
              tabs={tabs}
              activeTabId={activeTabId}
              onTabClick={setActiveTab}
              onTabClose={handleRemoveTab}
              onNewTab={handleNewTab}
              onTabMove={moveTab}
              onTabDragMove={handleTabDragMove}
              onTabDrop={handleTabDrop}
              onTabDragCancel={handleTabDragCancel}
              onContextMenu={handleTabContextMenu}
            />
            <div className="tab-add-button">
              <button
                className="tab-action"
                onClick={handleNewTab}
                aria-label={t("tabs.newTab", { defaultValue: "New tab" })}
              >
                <Plus size={16} />
              </button>
              <button
                className="tab-action"
                onClick={() => setShowProfilesPanel(true)}
                aria-label={t("profiles.title", { defaultValue: "Profiles" })}
              >
                <BookMarked size={16} />
              </button>
              <TransferManager
                transfers={transfers}
                onCancel={cancelTransfer}
                onRemove={removeTransfer}
                onClearCompleted={clearCompletedTransfers}
              />
            </div>
          </div>
        </div>

        <div className="drag-space" data-tauri-drag-region></div>

        <div className="title-bar-right" style={{ paddingRight: `${nativeControlsReservePx}px` }}>
          <button
            className="tab-action settings-button"
            onClick={handleSettingsClick}
            aria-label={settingsTabTitle}
          >
            <Settings size={16} />
          </button>
          {isLinux && (
            <div className="window-controls" aria-label="Window controls">
              <button
                className="window-control-button"
                onClick={handleMinimizeWindow}
                aria-label={t("window.minimize", { defaultValue: "Minimize" })}
              >
                <Minus size={16} />
              </button>
              <button
                className="window-control-button"
                onClick={handleToggleMaximizeWindow}
                aria-label={t("window.maximize", { defaultValue: "Maximize" })}
              >
                <Square size={13} />
              </button>
              <button
                className="window-control-button close"
                onClick={handleCloseWindow}
                aria-label={t("window.close", { defaultValue: "Close" })}
              >
                <X size={16} />
              </button>
            </div>
          )}
        </div>
      </div>

      <div className="content-area">{renderTabContent()}</div>

      {showConnectionDialog && (
        <ConnectionDialog
          isOpen={showConnectionDialog}
          onClose={() => {
            setShowConnectionDialog(false)
            setEditingProfile(null)
            setDuplicatingProfile(null)
            setProfilesRefreshKey((key) => key + 1)
          }}
          onConnect={handleConnect}
          editProfile={editingProfile}
          duplicateProfile={duplicatingProfile}
        />
      )}

      <Dialog open={showProfilesPanel} onOpenChange={setShowProfilesPanel}>
        <DialogContent
          showCloseButton={false}
          className="flex h-[min(720px,85vh)] flex-col overflow-hidden border-0 p-0 shadow-none sm:max-w-3xl"
        >
          <ProfilesPanel
            refreshKey={profilesRefreshKey}
            collapsedGroupKeys={config.collapsed_profile_group_keys}
            surface="panel"
            onCollapsedGroupKeysChange={handleCollapsedProfileGroupKeysChange}
            onClose={() => setShowProfilesPanel(false)}
            onCreate={() => {
              setEditingProfile(null)
              setDuplicatingProfile(null)
              setShowConnectionDialog(true)
              setShowProfilesPanel(false)
            }}
            onConnect={(connection) => {
              handleConnect(connection)
              setShowProfilesPanel(false)
            }}
            onEdit={(profile) => {
              handleEditProfile(profile)
              setShowProfilesPanel(false)
            }}
            onDuplicate={(profile) => {
              handleDuplicateProfile(profile)
              setShowProfilesPanel(false)
            }}
            onDeleteProfile={handleDeleteProfile}
          />
        </DialogContent>
      </Dialog>

      {contextMenu.visible && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          actions={contextMenu.actions}
          onAction={handleContextMenuAction}
          onClose={handleCloseContextMenu}
        />
      )}

      <ConfirmDialog />

      <RenameDialog
        isOpen={renameDialogState.isOpen}
        currentName={renameDialogState.currentName}
        onConfirm={handleRenameConfirm}
        onClose={handleRenameClose}
      />

      <VaultStartupUnlockDialog
        open={shouldPromptStartupVaultUnlock}
        onClose={() => setStartupVaultUnlockDismissed(true)}
      />
    </div>
  )
}
