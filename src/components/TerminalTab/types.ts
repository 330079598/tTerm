import type { Tab } from "@/types/tab"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import type { DockviewPanelApi } from "dockview-react"
import type { TerminalInputRequest } from "@/types/broadcast"
import type { LiveBroadcastState } from "@/types/broadcast"

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
  onSessionUnavailable?: (tabId: string) => void
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
