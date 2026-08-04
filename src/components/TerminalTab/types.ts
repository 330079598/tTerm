import type { Tab } from "@/types/tab"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import type { DockviewPanelApi } from "dockview-react"
import type { TerminalInputRequest } from "@/types/broadcast"
import type { LiveBroadcastState } from "@/types/broadcast"
import type { ExecutedCommand } from "@/types/command"

export interface TerminalTabProps {
  tabId: string
  sessionNonce?: number
  isActive: boolean
  workspacePanelApi?: Pick<DockviewPanelApi, "onDidDimensionsChange">
  connectionHeaderPinned?: boolean
  connection?: Tab["connection"]
  onPidChange?: (pid: number) => void
  onConnectionStateChange?: (
    tabId: string,
    sessionNonce: number,
    state: ConnectionState | null
  ) => void
  onInput?: (request: TerminalInputRequest) => Promise<void>
  onCommandExecuted?: (command: ExecutedCommand) => void
  onOpenCommandLibrary?: (query?: string) => void
  onSaveCommand?: (commandText: string, profile?: { id: string; name: string }) => void
  onSavedPasswordPromptChange?: (tabId: string, sessionNonce: number, active: boolean) => void
  onSessionUnavailable?: (tabId: string, sessionNonce: number, unexpected: boolean) => void
  onSensitivePrompt?: (tabId: string) => void
  onReconnectRequest?: () => void
  onOpenRemoteFile?: (
    entry: SftpDirectoryEntry,
    sourceTabId: string,
    connection?: Tab["connection"]
  ) => void
  onPinConnectionHeader?: () => void
  onServerMonitorVisibilityChange?: (visible: boolean) => void
  onUnpinConnectionHeader?: () => void
  isBroadcastSource?: boolean
  liveBroadcastState?: LiveBroadcastState
  onPauseBroadcast?: () => void
  onResumeBroadcast?: () => void
  onStopBroadcast?: () => void
}

export type ConnectionState = "connecting" | "connected" | "disconnected" | "error"

export type SshConnectionProgress = {
  phase: string
  message: string
  hopIndex?: number
  totalHops?: number
  host?: string
  port?: number
  username?: string
  networkLatencyMs?: number
}

export type HostKeyPromptState = {
  requestId: string
  profileName: string
  host: string
  port: number
  algorithm: string
  fingerprint: string
  reason: string
  knownFingerprint?: string
}
