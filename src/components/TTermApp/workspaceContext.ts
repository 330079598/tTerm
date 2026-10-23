// The workspace's props and the context its panes and their headers read them
// from, apart from TabPanels so each pane component can live in its own file.

import type React from "react"
import { createContext, useContext } from "react"

import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import type { ConnectionState, SshConnectionProgress } from "@/components/TerminalTab/types"
import type { SplitDirection, TabGroup } from "@/lib/tabGroups"
import type { PaneSettings } from "@/lib/tabSettings"
import type { TerminalAttentionEvent } from "@/lib/terminalNotifications"
import type { LiveBroadcastState, TerminalInputRequest } from "@/types/broadcast"
import type { ExecutedCommand } from "@/types/command"
import type { SavedProfile, Tab, TabContextMenuAction } from "@/types/tab"

export interface TabPanelsProps {
  activeTabId: string | null
  groups: TabGroup[]
  /** Whether split panes show headers, and how much the ones without the keyboard fade. */
  paneLook: Pick<PaneSettings, "show_headers" | "dim_inactive_percent">
  duplicateTab: (id: string, split?: SplitDirection) => string | null
  mergeGroup: (id: string, targetId: string, direction: SplitDirection) => void
  placePane: (id: string, targetId: string, direction: SplitDirection) => void
  breakOutPane: (id: string, index?: number) => void
  ungroupPanes: (id: string) => void
  togglePaneZoom: (id: string) => void
  resizeSplit: (groupId: string, path: number[], sizes: number[]) => void
  handlePinConnectionHeader: (tabId: string) => void
  handleReconnectTab: (tabId: string) => void
  handleServerMonitorVisibilityChange: (tabId: string, visible: boolean) => void
  handleUnpinConnectionHeader: (tabId: string) => void
  onActiveTabChange: (tabId: string) => void
  onOpenRemoteFile: (
    entry: SftpDirectoryEntry,
    sourceTabId: string,
    connection?: Tab["connection"]
  ) => void
  onConnectProfile: (connection: Omit<Tab, "id" | "isActive">) => void
  onEditProfile: (profile: SavedProfile) => void
  /** Closes one pane; closing a group's last pane closes its tab in the bar. */
  onTabClose: (tabId: string) => void
  onTabContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
  onTerminalConnectionStateChange: (
    tabId: string,
    sessionNonce: number,
    state: ConnectionState | null
  ) => void
  onTerminalInput: (request: TerminalInputRequest) => Promise<boolean | void>
  onTerminalCommandExecuted?: (command: ExecutedCommand) => void
  onTerminalAttention?: (tabId: string, event: TerminalAttentionEvent) => void
  onOpenCommandLibrary?: (query?: string) => void
  onSaveCommand?: (commandText: string, profile?: { id: string; name: string }) => void
  onTerminalSavedPasswordPromptChange: (
    tabId: string,
    sessionNonce: number,
    prompt: string | null
  ) => void
  onTerminalSessionUnavailable: (tabId: string, sessionNonce: number, unexpected: boolean) => void
  onTerminalSensitivePrompt: (tabId: string) => void
  onTerminalConnectionProgress?: (tabId: string, progress: SshConnectionProgress) => void
  profilesRefreshKey?: number
  startupConnectionsReady: boolean
  startupSessionRestoreMode: "active" | "all"
  tabs: Tab[]
  broadcastSourceTabId: string | null
  liveBroadcastState: LiveBroadcastState
  onPauseBroadcast: () => void
  onResumeBroadcast: () => void
  onStopBroadcast: () => void
  updateTab: (id: string, updater: (tab: Tab) => Tab) => void
}

export type WorkspaceContextValue = Omit<
  TabPanelsProps,
  "activeTabId" | "groups" | "mergeGroup" | "placePane" | "resizeSplit" | "paneLook"
> & {
  /** Starts dragging a pane by its header, to another place or out into a tab of its own. */
  startPaneDrag: (event: React.PointerEvent<HTMLElement>, tabId: string) => void
}

export const WorkspaceContext = createContext<WorkspaceContextValue | null>(null)

export function useWorkspace() {
  const value = useContext(WorkspaceContext)
  if (!value) {
    throw new Error("Workspace components must be rendered inside TabPanels")
  }
  return value
}
