import type { BroadcastState } from "@/components/TTermApp/useBroadcastState"
import type { BroadcastRuntime } from "@/components/TTermApp/useBroadcastRuntime"
import type { useBroadcastControls } from "@/components/TTermApp/useBroadcastControls"
import type { useBroadcastSending } from "@/components/TTermApp/useBroadcastSending"
import type { useTitleBarPages } from "@/components/TTermApp/useTitleBarPages"
import type { useCommandLibraryActions } from "@/components/TTermApp/useCommandLibraryActions"
import type { useWorkspaceSplits } from "@/components/TTermApp/useWorkspaceSplits"
import type { useTabContextMenu } from "@/hooks/useTabContextMenu"
import type { useTransferManager } from "@/contexts/TransferContext"
import type { Tab } from "@/types/tab"
import type { TabGroup } from "@/lib/tabGroups"
import { BroadcastManager } from "@/components/BroadcastManager"
import { TransferManager } from "@/components/TransferManager"
import { TitleBarActions } from "@/components/TTermApp/TitleBarActions"
import { TitleBarTabs } from "@/components/TTermApp/TitleBarTabs"

type TransferManager = ReturnType<typeof useTransferManager>

interface AppTitleBarProps {
  tabs: Tab[]
  groups: TabGroup[]
  activeTabId: string | null
  setActiveTab: (id: string) => void
  handleCloseTab: (id: string) => Promise<void>
  handleNewTab: () => void
  moveTab: (fromIndex: number, toIndex: number) => void
  handleTabDragMove: ReturnType<typeof useWorkspaceSplits>["handleTabDragMove"]
  handleTabDrop: ReturnType<typeof useWorkspaceSplits>["handleTabDrop"]
  handleTabDragCancel: () => void
  handleTabContextMenu: ReturnType<typeof useTabContextMenu>["handleTabContextMenu"]
  setShowProfilesPanel: (show: boolean) => void
  transfers: TransferManager["transfers"]
  cancelTransfer: TransferManager["cancelTransfer"]
  removeTransfer: TransferManager["removeTransfer"]
  retryTransfer: TransferManager["retryTransfer"]
  clearCompletedTransfers: TransferManager["clearCompletedTransfers"]
  nativeControlsReservePx: number
  runningTunnelCount: number
  settingsTabTitle: string
  isLinux: boolean
  handleMinimizeWindow: () => void
  handleToggleMaximizeWindow: () => void
  handleCloseWindow: () => void
  broadcast: BroadcastState
  controls: ReturnType<typeof useBroadcastControls>
  sending: ReturnType<typeof useBroadcastSending>
  runtime: BroadcastRuntime
  titleBar: ReturnType<typeof useTitleBarPages>
  commandLibrary: ReturnType<typeof useCommandLibraryActions>
}

/** The title bar: the tabs, the tools beside them, and the page and window buttons. */
export function AppTitleBar({
  tabs,
  groups,
  activeTabId,
  setActiveTab,
  handleCloseTab,
  handleNewTab,
  moveTab,
  handleTabDragMove,
  handleTabDrop,
  handleTabDragCancel,
  handleTabContextMenu,
  setShowProfilesPanel,
  transfers,
  cancelTransfer,
  removeTransfer,
  retryTransfer,
  clearCompletedTransfers,
  nativeControlsReservePx,
  runningTunnelCount,
  settingsTabTitle,
  isLinux,
  handleMinimizeWindow,
  handleToggleMaximizeWindow,
  handleCloseWindow,
  broadcast,
  controls,
  sending,
  runtime,
  titleBar,
  commandLibrary,
}: AppTitleBarProps) {
  return (
    <div className="title-bar">
      <TitleBarTabs
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
        onOpenCommandLibrary={commandLibrary.handleOpenCommandLibrary}
      >
        <TransferManager
          transfers={transfers}
          onCancel={cancelTransfer}
          onRemove={removeTransfer}
          onRetry={retryTransfer}
          onClearCompleted={clearCompletedTransfers}
        />
        <BroadcastManager
          activeTabId={activeTabId}
          mode={broadcast.broadcastMode}
          liveState={broadcast.liveBroadcastState}
          preparing={broadcast.isBroadcastPreparing}
          latestResults={broadcast.latestBroadcastResults}
          unavailableTabIds={broadcast.unavailableBroadcastTargetIds}
          runtimeStates={broadcast.terminalRuntimeStates}
          selectedTabIds={broadcast.broadcastTargetIds}
          sourceTabId={broadcast.broadcastSource?.tabId ?? null}
          tabs={tabs}
          onClear={controls.clearBroadcastTargets}
          onModeChange={broadcast.setBroadcastMode}
          onReconnectTargets={controls.reconnectBroadcastTargets}
          onSelectAll={controls.selectAllBroadcastTargets}
          onSelectVisible={controls.selectVisibleBroadcastTargets}
          onSendCommand={sending.handleSendBroadcastCommand}
          onStartLive={sending.handleStartLiveBroadcast}
          onStopLive={runtime.stopLiveBroadcast}
          onToggleTarget={controls.toggleBroadcastTarget}
        />
      </TitleBarTabs>

      <div className="drag-space" data-tauri-drag-region></div>

      <TitleBarActions
        nativeControlsReservePx={nativeControlsReservePx}
        runningTunnelCount={runningTunnelCount}
        settingsTabTitle={settingsTabTitle}
        tunnelsButtonLabel={titleBar.tunnelsButtonLabel}
        isLinux={isLinux}
        onTunnelsClick={titleBar.handleTunnelsClick}
        onSettingsClick={titleBar.handleSettingsClick}
        onMinimizeWindow={handleMinimizeWindow}
        onToggleMaximizeWindow={handleToggleMaximizeWindow}
        onCloseWindow={handleCloseWindow}
      />
    </div>
  )
}
