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
  isGlobalShortcutTarget: boolean
  workspacePanelApi?: Pick<DockviewPanelApi, "onDidDimensionsChange">
  connectionHeaderPinned?: boolean
  connection?: Tab["connection"]
  onPidChange?: (pid: number) => void
  onConnectionStateChange?: (
    tabId: string,
    sessionNonce: number,
    state: ConnectionState | null
  ) => void
  /** Resolves true for a "saved-password" request whose password was written. */
  onInput?: (request: TerminalInputRequest) => Promise<boolean | void>
  onCommandExecuted?: (command: ExecutedCommand) => void
  onOpenCommandLibrary?: (query?: string) => void
  onSaveCommand?: (commandText: string, profile?: { id: string; name: string }) => void
  /** Reports the prompt a saved password can answer, or null once it is gone. */
  onSavedPasswordPromptChange?: (tabId: string, sessionNonce: number, prompt: string | null) => void
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

/** Which saved password answers a sudo prompt (see `get_sudo_password_source`). */
export type SudoPasswordSource = "sudo" | "login"

export type SavedPasswordPromptState =
  | { status: "available"; prompt: string; user: string | null; source: SudoPasswordSource }
  /** The saved password was just refused; autofill is paused for the session. */
  | { status: "rejected"; prompt: string; user: string | null }

export type SavedPasswordPromptActions = {
  /** Sends the saved password; false when no prompt is on offer. */
  fill: () => boolean
  /** Hides the offer for the current prompt. */
  dismiss: () => void
  /** True while the cursor sits on a password prompt, offered or not. */
  atPasswordPrompt: () => boolean
}

export type ConnectionState = "connecting" | "connected" | "disconnected" | "reconnecting" | "error"

export type SshConnectionProgress = {
  phase: string
  message: string
  hopIndex?: number
  totalHops?: number
  host?: string
  port?: number
  username?: string
  networkLatencyMs?: number
  retryAttempt?: number
  retryDelaySecs?: number
  retryMaxAttempts?: number
  reason?: string
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
