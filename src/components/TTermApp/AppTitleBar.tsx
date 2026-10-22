import { useMemo } from "react"
import { useTranslation } from "react-i18next"
import { BookMarked, Library, Minus, Plus, Settings, Square, Waypoints, X } from "lucide-react"

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
import { getPaneIds, type TabGroup } from "@/lib/tabGroups"
import { BroadcastManager } from "@/components/BroadcastManager"
import { TabBar } from "@/components/TabBar"
import { TransferManager } from "@/components/TransferManager"
import { useShortcutLabel } from "@/contexts/KeymapContext"
import { withShortcut } from "@/lib/keymap/keymap"
import { keepFocusOnMouseDown } from "@/lib/terminalFocus"

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
  const { t } = useTranslation()
  const shortcutLabel = useShortcutLabel()
  const newTabLabel = t("tabs.newTab", { defaultValue: "New tab" })

  // The bar shows each group as the pane it had the focus in, with its other panes behind it.
  const { barTabs, tabPanes } = useMemo(() => {
    const byId = new Map(tabs.map((tab) => [tab.id, tab]))
    return {
      barTabs: groups.flatMap((group) => byId.get(group.activeTabId) ?? []),
      tabPanes: new Map(
        groups.map((group) => [
          group.activeTabId,
          {
            tabs: getPaneIds(group.root).flatMap((id) => byId.get(id) ?? []),
            isZoomed: group.zoomedTabId !== undefined,
          },
        ])
      ),
    }
  }, [groups, tabs])

  return (
    <div className="title-bar">
      <div className="title-bar-left">
        <div className="tab-list-container">
          <TabBar
            tabs={barTabs}
            tabPanes={tabPanes}
            activeTabId={activeTabId}
            onTabClick={setActiveTab}
            onTabClose={handleCloseTab}
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
              onMouseDown={keepFocusOnMouseDown}
              onClick={handleNewTab}
              aria-label={newTabLabel}
              title={withShortcut(newTabLabel, shortcutLabel("workspace.newTab"))}
            >
              <Plus size={16} />
            </button>
            <button
              className="tab-action"
              onMouseDown={keepFocusOnMouseDown}
              onClick={() => setShowProfilesPanel(true)}
              aria-label={t("profiles.title", { defaultValue: "Profiles" })}
            >
              <BookMarked size={16} />
            </button>
            <button
              className="tab-action"
              onMouseDown={keepFocusOnMouseDown}
              onClick={() => commandLibrary.handleOpenCommandLibrary()}
              aria-label={t("commandLibrary.title")}
              title={withShortcut(t("commandLibrary.title"), shortcutLabel("commandLibrary.open"))}
            >
              <Library size={16} />
            </button>
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
          </div>
        </div>
      </div>

      <div className="drag-space" data-tauri-drag-region></div>

      <div className="title-bar-right" style={{ paddingRight: `${nativeControlsReservePx}px` }}>
        <button
          className="tab-action settings-button tunnels-trigger"
          onMouseDown={keepFocusOnMouseDown}
          onClick={titleBar.handleTunnelsClick}
          aria-label={titleBar.tunnelsButtonLabel}
          title={titleBar.tunnelsButtonLabel}
        >
          <Waypoints size={16} />
          {runningTunnelCount > 0 && (
            <span className="tunnels-running-badge">
              {runningTunnelCount > 99 ? "99+" : runningTunnelCount}
            </span>
          )}
        </button>
        <button
          className="tab-action settings-button"
          onMouseDown={keepFocusOnMouseDown}
          onClick={titleBar.handleSettingsClick}
          aria-label={settingsTabTitle}
        >
          <Settings size={16} />
        </button>
        {isLinux && (
          <div
            className="window-controls"
            aria-label={t("window.controls", { defaultValue: "Window controls" })}
          >
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
  )
}
