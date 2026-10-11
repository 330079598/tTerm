// The workspace's props and the context its dock panels and tab headers read
// them from, apart from TabPanels so each dockview component can live in its
// own file.

import type React from "react"
import { createContext, useContext, useEffect, useState } from "react"
import type { IDockviewPanelProps, SerializedDockview } from "dockview-react"

import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import type { ConnectionState, SshConnectionProgress } from "@/components/TerminalTab/types"
import type { TerminalAttentionEvent } from "@/lib/terminalNotifications"
import type { LiveBroadcastState, TerminalInputRequest } from "@/types/broadcast"
import type { ExecutedCommand } from "@/types/command"
import type { SavedProfile, Tab, TabContextMenuAction } from "@/types/tab"

export type WorkspacePanelParams = {
  tabId: string
}

export type WorkspaceSplitDirection = "left" | "right" | "above" | "below"

export interface TabPanelsProps {
  activeTabId: string | null
  duplicateTab: (id: string) => string | null
  handlePinConnectionHeader: (tabId: string) => void
  handleReconnectTab: (tabId: string) => void
  handleServerMonitorVisibilityChange: (tabId: string, visible: boolean) => void
  handleUnpinConnectionHeader: (tabId: string) => void
  initialLayout: SerializedDockview | null
  onActiveTabChange: (tabId: string) => void
  onLayoutChange: (layout: SerializedDockview) => void
  onOpenRemoteFile: (
    entry: SftpDirectoryEntry,
    sourceTabId: string,
    connection?: Tab["connection"]
  ) => void
  onConnectProfile: (connection: Omit<Tab, "id" | "isActive">) => void
  onEditProfile: (profile: SavedProfile) => void
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
  "activeTabId" | "initialLayout" | "onActiveTabChange" | "onLayoutChange"
> & {
  collapseToSingleGroup: (tabId: string) => void
  groupCount: number
  maximizedGroupId: string | null
  splitTab: (tabId: string, direction: WorkspaceSplitDirection) => void
}

export const WorkspaceContext = createContext<WorkspaceContextValue | null>(null)

export function useWorkspace() {
  const value = useContext(WorkspaceContext)
  if (!value) {
    throw new Error("Workspace components must be rendered inside TabPanels")
  }
  return value
}

export function useDockPanelState(api: IDockviewPanelProps["api"]) {
  const [state, setState] = useState(() => ({
    hasBeenVisible: api.isVisible,
    isActive: api.isActive,
    isVisible: api.isVisible,
  }))

  useEffect(() => {
    const update = () => {
      setState((current) => ({
        hasBeenVisible: current.hasBeenVisible || api.isVisible,
        isActive: api.isActive,
        isVisible: api.isVisible,
      }))
    }
    const disposables = [api.onDidActiveChange(update), api.onDidVisibilityChange(update)]
    update()
    return () => disposables.forEach((disposable) => disposable.dispose())
  }, [api])

  return state
}
