import type { DistroId } from "@/lib/distro"
import type { SshAuthUsed, Tab } from "@/types/tab"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import type { TerminalInputRequest } from "@/types/broadcast"
import type { LiveBroadcastState } from "@/types/broadcast"
import type { ExecutedCommand } from "@/types/command"
import type { LocalShellChoice } from "@/lib/localShells"
import type { TerminalAttentionEvent } from "@/lib/terminalNotifications"

export interface TerminalTabProps {
  tabId: string
  sessionNonce?: number
  isActive: boolean
  isGlobalShortcutTarget: boolean
  connectionHeaderPinned?: boolean
  connection?: Tab["connection"]
  /** The shell a local tab runs (`pwsh`, `cmd`, `WSL`, `zsh`), for quoting dropped paths. */
  shellName?: string
  onPidChange?: (pid: number) => void
  /** A local shell reported a new working directory. */
  onCwdChange?: (cwd: string) => void
  /** A local WSL shell's launcher reported the distribution it runs. */
  onDistroChange?: (distroId: DistroId) => void
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
  /** Every progress event of the tab's own SSH connection. */
  onConnectionProgress?: (progress: SshConnectionProgress) => void
  /** A finished command, a notification request or a bell in this tab. */
  onAttention?: (tabId: string, event: TerminalAttentionEvent) => void
  onReconnectRequest?: () => void
  /** Starts a local tab again with another shell, after its own failed to start. */
  onRestartWithShell?: (choice: LocalShellChoice) => void
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
  /**
   * No saved password is offered; the user types it. `typed` counts the
   * characters so far, which the remote does not echo.
   */
  | { status: "manual"; prompt: string; user: string | null; typed: number }

export type SavedPasswordPromptActions = {
  /** Sends the saved password; false when no prompt is on offer. */
  fill: () => boolean
  /** Hides the bar for the current prompt, typing hint included. */
  dismiss: () => void
  /** True while the cursor sits on a password prompt, offered or not. */
  atPasswordPrompt: () => boolean
}

export type ConnectionState = "connecting" | "connected" | "disconnected" | "reconnecting" | "error"

/** Why an SSH session with shell integration turned on got a plain shell. */
export type ShellIntegrationUnavailableReason =
  | { code: "shell"; shell: string }
  | {
      code: "noReply" | "writeFailed" | "path" | "rejected" | "timedOut" | "failed"
    }

export type ShellIntegrationStatus =
  /** `hostname` is the host's own name (`uname -n`), when it gave one. */
  | { status: "active"; hostname?: string }
  | { status: "unavailable"; reason: ShellIntegrationUnavailableReason }

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
  /** `ready` phase of a connection with shell integration turned on. */
  shellIntegration?: ShellIntegrationStatus
  /** `target_authenticated` phase of a quick connection: what got the user in. */
  auth?: SshAuthUsed
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
