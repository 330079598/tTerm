import "@/components/TTermApp.css"
import { platform } from "@tauri-apps/plugin-os"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { ConnectionDialog } from "@/components/ConnectionDialog"
import { CommandLibrary } from "@/components/CommandLibrary"
import { CommandEditorDialog } from "@/components/CommandLibrary/CommandEditorDialog"
import { CloseRequestDialog } from "@/components/CloseRequestDialog"
import { ContextMenu } from "@/components/ContextMenu"
import { NewTabLauncher } from "@/components/NewTabLauncher"
import { ProfilesPanel } from "@/components/ProfilesPanel"
import { RenameDialog } from "@/components/RenameDialog"
import { AppTitleBar } from "@/components/TTermApp/AppTitleBar"
import { EmptyState } from "@/components/TTermApp/EmptyState"
import { TabPanels, type TabPanelsHandle } from "@/components/TTermApp/TabPanels"
import type { AppShell } from "@/components/TTermApp/appShell"
import { useBroadcastState } from "@/components/TTermApp/useBroadcastState"
import { useBroadcastRuntime } from "@/components/TTermApp/useBroadcastRuntime"
import { useTerminalLogRecording } from "@/components/TTermApp/useTerminalLogRecording"
import { useBroadcastSending } from "@/components/TTermApp/useBroadcastSending"
import { useTabConnectionActions } from "@/components/TTermApp/useTabConnectionActions"
import { useBroadcastControls } from "@/components/TTermApp/useBroadcastControls"
import { usePageTabs } from "@/components/TTermApp/usePageTabs"
import { useSessionRestore } from "@/components/TTermApp/useSessionRestore"
import { useTabOpeners } from "@/components/TTermApp/useTabOpeners"
import { useProfileDialogState } from "@/components/TTermApp/useProfileDialogState"
import { useQuickConnect } from "@/components/TTermApp/useQuickConnect"
import { useProfileActions } from "@/components/TTermApp/useProfileActions"
import { useTabClosing } from "@/components/TTermApp/useTabClosing"
import { useWorkspaceSplits } from "@/components/TTermApp/useWorkspaceSplits"
import { useTitleBarPages } from "@/components/TTermApp/useTitleBarPages"
import { useCommandLibraryActions } from "@/components/TTermApp/useCommandLibraryActions"
import { useTabKeymap } from "@/components/TTermApp/useTabKeymap"
import { TunnelHostKeyPrompt } from "@/components/TunnelsPanel/TunnelHostKeyPrompt"
import { useTunnelAutoStart } from "@/components/TunnelsPanel/useTunnelAutoStart"
import { useTunnelNotifications } from "@/components/TunnelsPanel/useTunnelNotifications"
import { useTunnelQuitGuard } from "@/components/TunnelsPanel/useTunnelQuitGuard"
import { useRunningTunnelCount } from "@/components/TunnelsPanel/useRunningTunnelCount"
import { VaultStartupUnlockDialog } from "@/components/VaultStartupUnlockDialog"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { useConfig } from "@/contexts/ConfigContext"
import { useAppActivity } from "@/contexts/AppActivityContext"
import { useTransferManager } from "@/contexts/TransferContext"
import { useConnectionManager } from "@/hooks/useConnectionManager"
import { useLatestRef } from "@/hooks/useLatestRef"
import { usePreloadedSession } from "@/hooks/usePreloadedSession"
import { useSessionPersistence } from "@/hooks/useSessionPersistence"
import { useTabContextMenu } from "@/hooks/useTabContextMenu"
import { useTabs } from "@/hooks/useTabs"
import { useTerminalFontZoom } from "@/hooks/useTerminalFontZoom"
import { useTerminalNotifications } from "@/hooks/useTerminalNotifications"
import { useWindowControls } from "@/hooks/useWindowControls"
import { onSyncApplied } from "@/lib/sync"
import { useWebDavSync } from "@/hooks/useWebDavSync"
const SETTINGS_TAB_TITLE = "Settings"

function getRuntimePlatform() {
  try {
    return platform()
  } catch {
    return "windows"
  }
}

export const TTermApp: React.FC = () => {
  const { t, i18n } = useTranslation()
  const { setActiveTerminalSessionCount } = useAppActivity()
  const [os] = useState<string>(getRuntimePlatform)
  const isMacos = os === "macos"
  const isLinux = os === "linux"
  const isWindows = os === "windows"
  const dialog = useProfileDialogState()
  const [showNewTabLauncher, setShowNewTabLauncher] = useState(false)
  const [showProfilesPanel, setShowProfilesPanel] = useState(false)
  const [profilesRefreshKey, setProfilesRefreshKey] = useState(0)
  const [startupVaultUnlockDismissed, setStartupVaultUnlockDismissed] = useState(false)
  const broadcast = useBroadcastState()
  const workspaceRef = useRef<TabPanelsHandle>(null)

  const tabsApi = useTabs()
  const { tabs, activeTabId, setActiveTab, moveTab, duplicateTab, renameTab, updateTab } = tabsApi
  const tabsRef = useRef(tabs)
  const activeTabIdRef = useRef(activeTabId)
  useLatestRef(tabsRef, tabs)
  useLatestRef(activeTabIdRef, activeTabId)

  const activeTerminalSessionCount = tabs.reduce((count, tab) => {
    if (tab.type !== "terminal" && tab.type !== "ssh") {
      return count
    }

    const connectionState = broadcast.terminalRuntimeStates[tab.id]?.connectionState
    return connectionState === "connected" ||
      connectionState === "connecting" ||
      connectionState === "reconnecting"
      ? count + 1
      : count
  }, 0)

  useEffect(() => {
    setActiveTerminalSessionCount(activeTerminalSessionCount)
  }, [activeTerminalSessionCount, setActiveTerminalSessionCount])

  useWebDavSync()
  useEffect(
    () =>
      onSyncApplied(["profiles", "profileGroups"], () => setProfilesRefreshKey((key) => key + 1)),
    []
  )

  useEffect(
    () => () => {
      setActiveTerminalSessionCount(0)
    },
    [setActiveTerminalSessionCount]
  )

  const { saveSession, loadSession } = useSessionPersistence()
  const getPreloadedSession = usePreloadedSession(loadSession)
  const { cleanupConnection } = useConnectionManager()
  const { appState, config, isLoaded, isSecretStatusLoaded, saveAppState, secretStatus } =
    useConfig()
  const { cancelTransfer, clearCompletedTransfers, removeTransfer, retryTransfer, transfers } =
    useTransferManager()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const settingsTabTitle = t("settings.title", { defaultValue: SETTINGS_TAB_TITLE })
  const tunnelsTabTitle = t("tunnels.title", { defaultValue: "Port Forwarding" })
  const logsTabTitle = t("terminalLogs.title")
  // Ask for the password when saved passwords are locked but one can unlock
  // them: the old vault's password to finish moving it, the master password,
  // or the recovery password when the system credential store key is lost.
  const shouldPromptStartupVaultUnlock =
    isLoaded &&
    !secretStatus.unlocked &&
    !startupVaultUnlockDismissed &&
    (secretStatus.migrationPending ||
      (secretStatus.hasMasterPassword &&
        (secretStatus.storageMode === "system" ||
          (secretStatus.storageMode === "password" && config.prompt_unlock_vault_on_startup))))
  // SSH connections wait for startup to unlock saved passwords, then for the prompt.
  const startupConnectionsReady = isSecretStatusLoaded && !shouldPromptStartupVaultUnlock
  useTunnelAutoStart(isLoaded && startupConnectionsReady)
  useTunnelQuitGuard(confirm)
  useTunnelNotifications()
  const runningTunnelCount = useRunningTunnelCount()

  const shell: AppShell = {
    ...tabsApi,
    t,
    tabsRef,
    activeTabIdRef,
    workspaceRef,
    confirm,
    cleanupConnection,
  }
  const runtime = useBroadcastRuntime(shell, broadcast)
  const handleSetTabLogRecording = useTerminalLogRecording(t)
  const sending = useBroadcastSending(shell, broadcast, runtime)
  const tabConnection = useTabConnectionActions(shell)
  const controls = useBroadcastControls(shell, broadcast, runtime, tabConnection.handleReconnectTab)
  usePageTabs(shell, { isLoaded, config, i18n, settingsTabTitle, tunnelsTabTitle, logsTabTitle })
  const { workspaceLayout, setWorkspaceLayout } = useSessionRestore(shell, {
    isLoaded,
    getPreloadedSession,
    saveSession,
  })

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
    setShowNewTabLauncher(true)
  }, [])

  const { handleOpenLocalTerminal, handleConnect, handleOpenRemoteFile } = useTabOpeners(
    shell,
    config
  )
  const quickConnect = useQuickConnect(shell, dialog)
  const profileActions = useProfileActions(shell, dialog, {
    liveSourceRef: broadcast.liveSourceRef,
    runtimeStatesRef: broadcast.runtimeStatesRef,
    stopLiveSource: runtime.stopLiveSource,
    openSaveQuickConnectDialog: quickConnect.openSaveQuickConnectDialog,
    handleQuickConnectSaved: quickConnect.handleQuickConnectSaved,
    handleReconnectTab: tabConnection.handleReconnectTab,
  })
  const { handleRemoveTab, handleCloseOtherTabs, handleCloseTabsToLeft, handleCloseTabsToRight } =
    useTabClosing(shell, { transfers, stopLiveSource: runtime.stopLiveSource })
  const {
    handleSplitTab,
    handleTabDragMove,
    handleTabDrop,
    handleTabDragCancel,
    getTabContextIds,
  } = useWorkspaceSplits(shell)

  useTerminalFontZoom()

  const {
    nativeControlsReservePx,
    handleMinimizeWindow,
    handleToggleMaximizeWindow,
    handleCloseWindow,
  } = useWindowControls(isWindows)

  const {
    contextMenu,
    renameDialog,
    handleTabContextMenu,
    handleContextMenuAction,
    handleCloseContextMenu,
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
    editTabProfile: profileActions.handleEditTabProfile,
    setTabLogRecording: handleSetTabLogRecording,
  })

  const titleBar = useTitleBarPages(shell, {
    settingsTabTitle,
    tunnelsTabTitle,
    runningTunnelCount,
  })

  const getVisibleTerminalTabIds = useCallback(
    () => workspaceRef.current?.getVisibleTerminalTabIds() ?? [],
    []
  )
  const handleTerminalAttention = useTerminalNotifications({
    tabs,
    activeTabId,
    activateTab: setActiveTab,
    getVisibleTabIds: getVisibleTerminalTabIds,
  })

  const commandLibrary = useCommandLibraryActions(shell, broadcast)
  useTabKeymap(shell, { handleNewTab, handleRemoveTab })

  const handleCollapsedProfileGroupKeysChange = useCallback(
    (groups: string[]) => {
      saveAppState({ collapsedProfileGroupKeys: groups }).catch((error) => {
        console.error("Failed to save collapsed profile groups:", error)
      })
    },
    [saveAppState]
  )

  const renderTabContent = () => {
    if (tabs.length === 0) {
      return (
        <EmptyState
          collapsedProfileGroupKeys={appState.collapsedProfileGroupKeys}
          handleConnect={handleConnect}
          handleNewTab={handleNewTab}
          onCollapsedProfileGroupKeysChange={handleCollapsedProfileGroupKeysChange}
          onDuplicateProfile={profileActions.handleDuplicateProfile}
          onDeleteProfile={profileActions.handleDeleteProfile}
          onEditProfile={profileActions.handleEditProfile}
          refreshKey={profilesRefreshKey}
        />
      )
    }

    return (
      <TabPanels
        ref={workspaceRef}
        activeTabId={activeTabId}
        broadcastSourceTabId={
          broadcast.broadcastMode === "live" ? (broadcast.broadcastSource?.tabId ?? null) : null
        }
        liveBroadcastState={broadcast.liveBroadcastState}
        duplicateTab={duplicateTab}
        handlePinConnectionHeader={tabConnection.handlePinConnectionHeader}
        handleReconnectTab={tabConnection.handleReconnectTab}
        handleServerMonitorVisibilityChange={tabConnection.handleServerMonitorVisibilityChange}
        handleUnpinConnectionHeader={tabConnection.handleUnpinConnectionHeader}
        initialLayout={workspaceLayout}
        onActiveTabChange={setActiveTab}
        onConnectProfile={handleConnect}
        onEditProfile={profileActions.handleEditProfile}
        onLayoutChange={setWorkspaceLayout}
        onOpenRemoteFile={handleOpenRemoteFile}
        onPauseBroadcast={controls.pauseLiveBroadcast}
        onResumeBroadcast={controls.resumeLiveBroadcast}
        onStopBroadcast={runtime.stopLiveBroadcast}
        onTabClose={handleRemoveTab}
        onTabContextMenu={handleTabContextMenu}
        onTerminalConnectionStateChange={runtime.handleTerminalConnectionStateChange}
        onTerminalCommandExecuted={commandLibrary.handleTerminalCommandExecuted}
        onTerminalAttention={handleTerminalAttention}
        onTerminalInput={sending.handleTerminalInput}
        onOpenCommandLibrary={commandLibrary.handleOpenCommandLibrary}
        onSaveCommand={commandLibrary.handleSaveCommand}
        onTerminalSavedPasswordPromptChange={controls.handleTerminalSavedPasswordPromptChange}
        onTerminalSessionUnavailable={controls.handleTerminalSessionUnavailable}
        onTerminalSensitivePrompt={sending.handleTerminalSensitivePrompt}
        onTerminalConnectionProgress={quickConnect.handleTerminalConnectionProgress}
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
      <AppTitleBar
        tabs={tabs}
        activeTabId={activeTabId}
        setActiveTab={setActiveTab}
        handleRemoveTab={handleRemoveTab}
        handleNewTab={handleNewTab}
        moveTab={moveTab}
        handleTabDragMove={handleTabDragMove}
        handleTabDrop={handleTabDrop}
        handleTabDragCancel={handleTabDragCancel}
        getTabContextIds={getTabContextIds}
        handleTabContextMenu={handleTabContextMenu}
        setShowProfilesPanel={setShowProfilesPanel}
        transfers={transfers}
        cancelTransfer={cancelTransfer}
        removeTransfer={removeTransfer}
        retryTransfer={retryTransfer}
        clearCompletedTransfers={clearCompletedTransfers}
        nativeControlsReservePx={nativeControlsReservePx}
        runningTunnelCount={runningTunnelCount}
        settingsTabTitle={settingsTabTitle}
        isLinux={isLinux}
        handleMinimizeWindow={handleMinimizeWindow}
        handleToggleMaximizeWindow={handleToggleMaximizeWindow}
        handleCloseWindow={handleCloseWindow}
        broadcast={broadcast}
        controls={controls}
        sending={sending}
        runtime={runtime}
        titleBar={titleBar}
        commandLibrary={commandLibrary}
      />

      <div className="content-area">{renderTabContent()}</div>

      {dialog.showConnectionDialog && (
        <ConnectionDialog
          isOpen={dialog.showConnectionDialog}
          onClose={() => {
            dialog.setShowConnectionDialog(false)
            dialog.setEditingProfile(null)
            dialog.setEditingProfileTabId(null)
            dialog.setDuplicatingProfile(null)
            dialog.setDraftProfile(null)
            dialog.setSavingQuickConnectTabId(null)
            setProfilesRefreshKey((key) => key + 1)
          }}
          onConnect={handleConnect}
          editProfile={dialog.editingProfile}
          duplicateProfile={dialog.duplicatingProfile}
          draftProfile={dialog.draftProfile}
          saveOnly={dialog.savingQuickConnectTabId !== null}
          onSaved={profileActions.handleProfileSaved}
          typedPasswordTabId={dialog.savingQuickConnectTabId ?? undefined}
          canReconnect={dialog.editingProfileTabId !== null}
        />
      )}

      <NewTabLauncher
        open={showNewTabLauncher}
        onClose={() => setShowNewTabLauncher(false)}
        onConnect={handleConnect}
        onOpenLocalTerminal={handleOpenLocalTerminal}
        onOpenConnectionDialog={quickConnect.handleOpenConnectionDialogFromLauncher}
        recents={quickConnect.quickConnectRecents}
        onForgetRecent={quickConnect.handleForgetQuickConnectRecent}
        onClearRecents={quickConnect.handleClearQuickConnectRecents}
      />

      <Dialog open={showProfilesPanel} onOpenChange={setShowProfilesPanel}>
        <DialogContent
          showCloseButton={false}
          className="flex h-[min(720px,85vh)] flex-col overflow-hidden border-0 p-0 shadow-none sm:max-w-3xl"
        >
          <ProfilesPanel
            refreshKey={profilesRefreshKey}
            collapsedGroupKeys={appState.collapsedProfileGroupKeys}
            surface="panel"
            onCollapsedGroupKeysChange={handleCollapsedProfileGroupKeysChange}
            onClose={() => setShowProfilesPanel(false)}
            onCreate={() => {
              dialog.setEditingProfile(null)
              dialog.setDuplicatingProfile(null)
              dialog.setShowConnectionDialog(true)
              setShowProfilesPanel(false)
            }}
            onConnect={(connection) => {
              handleConnect(connection)
              setShowProfilesPanel(false)
            }}
            onEdit={(profile) => {
              profileActions.handleEditProfile(profile)
              setShowProfilesPanel(false)
            }}
            onDuplicate={(profile) => {
              profileActions.handleDuplicateProfile(profile)
              setShowProfilesPanel(false)
            }}
            onDeleteProfile={profileActions.handleDeleteProfile}
          />
        </DialogContent>
      </Dialog>

      <CommandLibrary
        open={commandLibrary.showCommandLibrary}
        onOpenChange={commandLibrary.setShowCommandLibrary}
        activeProfile={commandLibrary.activeCommandProfile}
        canInsert={commandLibrary.canInsertSavedCommand}
        onInsert={commandLibrary.handleInsertSavedCommand}
        onInsertRecent={commandLibrary.handleInsertCommandText}
        recentCommands={commandLibrary.recentCommands}
        initialSearchQuery={commandLibrary.commandLibrarySearchQuery}
      />

      <CommandEditorDialog
        open={Boolean(commandLibrary.quickSaveRequest)}
        command={null}
        draft={commandLibrary.quickSaveRequest?.draft}
        availableTags={[]}
        activeProfile={commandLibrary.quickSaveRequest?.profile}
        onOpenChange={(open) => {
          if (!open) commandLibrary.setQuickSaveRequest(null)
        }}
        onSaved={() => commandLibrary.setQuickSaveRequest(null)}
      />

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
      <CloseRequestDialog runningTunnelCount={runningTunnelCount} />

      <RenameDialog {...renameDialog} />

      <TunnelHostKeyPrompt />
      <VaultStartupUnlockDialog
        open={shouldPromptStartupVaultUnlock}
        onClose={() => setStartupVaultUnlockDismissed(true)}
      />
    </div>
  )
}
