import type { Tab } from "@/types/tab"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

export interface TerminalTabProps {
  tabId: string
  sessionNonce?: number
  isActive: boolean
  connectionHeaderPinned?: boolean
  connection?: Tab["connection"]
  onPidChange?: (pid: number) => void
  onReconnectRequest?: () => void
  onOpenRemoteFile?: (
    entry: SftpDirectoryEntry,
    sourceTabId: string,
    connection?: Tab["connection"]
  ) => void
  onPinConnectionHeader?: () => void
  onUnpinConnectionHeader?: () => void
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
