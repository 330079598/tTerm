import "@/components/TTermApp.css"
import React, { useCallback, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { CloseRequestDialog } from "@/components/CloseRequestDialog"
import { LayoutTemplateDialog } from "@/components/LayoutTemplateDialog"
import { ContextMenu } from "@/components/ContextMenu"
import { NewTabLauncher } from "@/components/NewTabLauncher"
import { ProxyCommandPrompt } from "@/components/ProxyCommandPrompt"
import { RenameDialog } from "@/components/RenameDialog"
import { AppTitleBar } from "@/components/TTermApp/AppTitleBar"
import { CommandLibraryDialogs } from "@/components/TTermApp/CommandLibraryDialogs"
import { ProfileDialogs } from "@/components/TTermApp/ProfileDialogs"
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
import { useConnectionIcons } from "@/components/TTermApp/useConnectionIcons"
import { useQuickConnect } from "@/components/TTermApp/useQuickConnect"
import { useProfileActions } from "@/components/TTermApp/useProfileActions"
import { useTabClosing } from "@/components/TTermApp/useTabClosing"
import { useWorkspaceSplits } from "@/components/TTermApp/useWorkspaceSplits"
import { useTitleBarPages } from "@/components/TTermApp/useTitleBarPages"
import { useCommandLibraryActions } from "@/components/TTermApp/useCommandLibraryActions"
import { useTabKeymap } from "@/components/TTermApp/useTabKeymap"
import { useLayoutTemplates } from "@/components/TTermApp/useLayoutTemplates"
import { useActiveTerminalSessionCount } from "@/components/TTermApp/useActiveTerminalSessionCount"
import {
  useBlockNativeContextMenu,
  useCollapsedProfileGroupsSaver,
  useProfilesRefreshKey,
  useRuntimePlatform,
} from "@/components/TTermApp/useAppChrome"
import { useStartupVaultUnlock } from "@/components/TTermApp/useStartupVaultUnlock"
import { TunnelHostKeyPrompt } from "@/components/TunnelsPanel/TunnelHostKeyPrompt"
import { useTunnelBackground } from "@/components/TunnelsPanel/useTunnelBackground"
import { VaultStartupUnlockDialog } from "@/components/VaultStartupUnlockDialog"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { useConfig } from "@/contexts/ConfigContext"
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
import { useWebDavSync } from "@/hooks/useWebDavSync"
import { getVisiblePaneIds } from "@/lib/tabGroups"
const SETTINGS_TAB_TITLE = "Settings"

export const TTermApp: React.FC = () => {
  const { t, i18n } = useTranslation()
  const { isMacos, isLinux, isWindows } = useRuntimePlatform()
  const dialog = useProfileDialogState()
  const [showNewTabLauncher, setShowNewTabLauncher] = useState(false)
  const [showProfilesPanel, setShowProfilesPanel] = useState(false)
  const broadcast = useBroadcastState()
  const workspaceRef = useRef<TabPanelsHandle>(null)

  const tabsApi = useTabs()
  const {
    tabs,
    groups,
    activeTabId,
    setActiveTab,
    moveTab,
    duplicateTab,
    renameTab,
    updateTab,
    breakOutPane,
    ungroupPanes,
    togglePaneZoom,
  } = tabsApi
  const tabsRef = useRef(tabs)
  const activeTabIdRef = useRef(activeTabId)
  useLatestRef(tabsRef, tabs)
  useLatestRef(activeTabIdRef, activeTabId)

  useActiveTerminalSessionCount(tabs, broadcast.terminalRuntimeStates)

  useWebDavSync()
  const { profilesRefreshKey, refreshProfiles } = useProfilesRefreshKey()

  const { saveSession, loadSession } = useSessionPersistence()
  const getPreloadedSession = usePreloadedSession(loadSession)
  const { cleanupConnection } = useConnectionManager()
  const { appState, config, isLoaded } = useConfig()
  const { cancelTransfer, clearCompletedTransfers, removeTransfer, retryTransfer, transfers } =
    useTransferManager()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const settingsTabTitle = t("settings.title", { defaultValue: SETTINGS_TAB_TITLE })
  const tunnelsTabTitle = t("tunnels.title", { defaultValue: "Port Forwarding" })
  const logsTabTitle = t("terminalLogs.title")
  const { shouldPromptStartupVaultUnlock, startupConnectionsReady, dismissStartupVaultUnlock } =
    useStartupVaultUnlock()
  const runningTunnelCount = useTunnelBackground(confirm, isLoaded && startupConnectionsReady)

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
  const { setTabIconStyle } = useConnectionIcons(shell)
  const sending = useBroadcastSending(shell, broadcast, runtime)
  const tabConnection = useTabConnectionActions(shell)
  const controls = useBroadcastControls(shell, broadcast, runtime, tabConnection.handleReconnectTab)
  usePageTabs(shell, { isLoaded, config, i18n, settingsTabTitle, tunnelsTabTitle, logsTabTitle })
  useSessionRestore(shell, {
    isLoaded,
    getPreloadedSession,
    saveSession,
    restoreTerminalContent: config.restore_terminal_content,
  })

  useBlockNativeContextMenu()

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
  const {
    handleRemoveTab,
    handleCloseTab,
    handleCloseOtherTabs,
    handleCloseTabsToLeft,
    handleCloseTabsToRight,
  } = useTabClosing(shell, { transfers, stopLiveSource: runtime.stopLiveSource })
  const { handleSplitTab, handleTabDragMove, handleTabDrop, handleTabDragCancel } =
    useWorkspaceSplits(shell)

  useTerminalFontZoom()
  const { openLayoutTemplates, layoutTemplateDialog } = useLayoutTemplates(shell)

  const {
    nativeControlsReservePx,
    handleMinimizeWindow,
    handleToggleMaximizeWindow,
    handleCloseWindow,
  } = useWindowControls(isWindows)

  const {
    contextMenu,
    renameDialog,
    openRenameDialog,
    handleTabContextMenu,
    handleContextMenuAction,
    handleCloseContextMenu,
  } = useTabContextMenu({
    handleNewTab,
    duplicateTab,
    splitTab: handleSplitTab,
    togglePaneZoom,
    breakOutPane,
    ungroupPanes,
    openLayoutTemplates,
    handleRemoveTab,
    handleCloseTab,
    handleCloseOtherTabs,
    handleCloseTabsToRight,
    handleCloseTabsToLeft,
    updateTab,
    renameTab,
    editTabProfile: profileActions.handleEditTabProfile,
    setTabLogRecording: handleSetTabLogRecording,
    setTabIconStyle,
  })

  const titleBar = useTitleBarPages(shell, {
    settingsTabTitle,
    tunnelsTabTitle,
    runningTunnelCount,
  })

  const getVisibleTerminalTabIds = useCallback(
    () => getVisiblePaneIds(groups, activeTabId),
    [activeTabId, groups]
  )
  const handleTerminalAttention = useTerminalNotifications({
    tabs,
    activeTabId,
    activateTab: setActiveTab,
    getVisibleTabIds: getVisibleTerminalTabIds,
  })

  const commandLibrary = useCommandLibraryActions(shell, broadcast)
  useTabKeymap(shell, { handleNewTab, handleRemoveTab, openRenameDialog })

  const handleCollapsedProfileGroupKeysChange = useCollapsedProfileGroupsSaver()

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
        groups={groups}
        paneLook={config.panes}
        mergeGroup={tabsApi.mergeGroup}
        placePane={tabsApi.placePane}
        breakOutPane={breakOutPane}
        ungroupPanes={ungroupPanes}
        togglePaneZoom={togglePaneZoom}
        resizeSplit={tabsApi.resizeSplit}
        broadcastSourceTabId={
          broadcast.broadcastMode === "live" ? (broadcast.broadcastSource?.tabId ?? null) : null
        }
        liveBroadcastState={broadcast.liveBroadcastState}
        duplicateTab={duplicateTab}
        handlePinConnectionHeader={tabConnection.handlePinConnectionHeader}
        handleReconnectTab={tabConnection.handleReconnectTab}
        handleServerMonitorVisibilityChange={tabConnection.handleServerMonitorVisibilityChange}
        handleUnpinConnectionHeader={tabConnection.handleUnpinConnectionHeader}
        onActiveTabChange={setActiveTab}
        onConnectProfile={handleConnect}
        onEditProfile={profileActions.handleEditProfile}
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
        groups={groups}
        activeTabId={activeTabId}
        setActiveTab={setActiveTab}
        handleCloseTab={handleCloseTab}
        handleNewTab={handleNewTab}
        moveTab={moveTab}
        handleTabDragMove={handleTabDragMove}
        handleTabDrop={handleTabDrop}
        handleTabDragCancel={handleTabDragCancel}
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

      <ProfileDialogs
        dialog={dialog}
        profileActions={profileActions}
        handleConnect={handleConnect}
        profilesRefreshKey={profilesRefreshKey}
        refreshProfiles={refreshProfiles}
        showProfilesPanel={showProfilesPanel}
        setShowProfilesPanel={setShowProfilesPanel}
        collapsedGroupKeys={appState.collapsedProfileGroupKeys}
        onCollapsedGroupKeysChange={handleCollapsedProfileGroupKeysChange}
      />

      <CommandLibraryDialogs commandLibrary={commandLibrary} />

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
      <LayoutTemplateDialog {...layoutTemplateDialog} />

      <TunnelHostKeyPrompt />
      <ProxyCommandPrompt />
      <VaultStartupUnlockDialog
        open={shouldPromptStartupVaultUnlock}
        onClose={dismissStartupVaultUnlock}
      />
    </div>
  )
}
