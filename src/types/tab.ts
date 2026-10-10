export interface SavedProfile {
  id: string
  name: string
  group: string
  connection_type: string
  host?: string
  port?: number
  username?: string
  password?: string
  ignore_saved_password?: boolean
  remember_password: boolean
  auth_method?: string
  agent_forward?: boolean
  private_key_path?: string
  private_key_passphrase?: string
  keepalive_interval_secs: number
  keepalive_count_max: number
  server_monitor_visible?: boolean
  use_jump_host?: boolean
  jump_hosts?: SavedJumpHost[]
  /** Offer a saved password at sudo prompts; missing means enabled. */
  sudo_autofill?: boolean
  /** Write-only: a new dedicated sudo password to store. */
  sudo_password?: string
  /** Write-only: remove the stored sudo password. */
  clear_sudo_password?: boolean
  /** Charset of the remote shell; missing means UTF-8. */
  encoding?: string
  /** Theme whose terminal colors the connection's tabs use; missing means the app theme. */
  terminal_theme?: string
  /** Commands typed into the shell once it is ready, one per line. */
  login_script?: string
  /** Install tTerm's shell integration on the host so its shell marks commands. */
  shell_integration?: boolean
  /** Whether the connection's sessions are logged; missing follows the logging setting. */
  session_log?: SessionLogPolicy
}

/** A connection's logging choice; `default` follows the logging setting. */
export type SessionLogPolicy = "always" | "never"

export interface SavedJumpHost {
  host: string
  port: number
  username: string
  auth_method: string
  private_key_path?: string
  private_key_passphrase?: string
  password?: string
}

export type TransferStatus = "pending" | "transferring" | "completed" | "failed" | "cancelled"
export type TransferDirection = "upload" | "download" | "delete"

export interface TransferTask {
  id: string
  tabId?: string
  batchId?: string
  direction: TransferDirection
  localPath: string
  remotePath: string
  fileName: string
  fileSize: number
  transferred: number
  status: TransferStatus
  error?: string
  startTime: number
  endTime?: number
  speed?: number
  /** Bytes already present when a resumed transfer started. */
  resumedFrom?: number
  /** Number of parallel SFTP channels used for this transfer. */
  parallelism?: number
  /** Completed without moving bytes: the destination was kept as it was. */
  skipped?: boolean
  /** Re-run the transfer, resuming from its checkpoint when possible. */
  retry?: () => void
  /** Cancel this transfer. Falls back to the SFTP cancel command when unset. */
  cancel?: () => void | Promise<void>
}

export type TerminalShellType =
  "auto" | "cmd" | "powershell" | "pwsh" | "wsl" | "git-bash" | "custom"
export type ConnectionType = "terminal" | "ssh"
export type TabType = ConnectionType | "settings" | "tunnels" | "logs" | "remote-file-editor"

/**
 * How an SSH host is authenticated. `interactive` stores nothing: the server's
 * prompts (password, one-time code) are answered when connecting.
 */
export type SshAuthMethod = "password" | "key" | "agent" | "interactive"

/**
 * How a connection authenticates: a profile's method, or `auto` for a quick
 * connection, which tries the agent, key files, then asks.
 */
export type ConnectionAuthMethod = SshAuthMethod | "auto"

/** The method automatic authentication found to work. */
export interface SshAuthUsed {
  method: SshAuthMethod
  privateKeyPath?: string
  /** Connecting the same way again needs nothing typed. */
  reusable: boolean
}

export interface JumpHostConnection {
  host: string
  port: number
  username: string
  authMethod: ConnectionAuthMethod
  password?: string
  privateKeyPath?: string
  privateKeyPassphrase?: string
}

export interface Tab {
  id: string
  title: string
  type: TabType
  isActive: boolean
  hasConnected?: boolean
  isModified?: boolean
  icon?: string
  pid?: number
  sessionNonce?: number
  connectionHeaderPinned?: boolean
  /** Set on a duplicated tab: the name its "-N" copies are numbered from. Cleared on rename. */
  duplicateBaseTitle?: string
  /** The title is a generated name (a local tab's shell): numbered when another tab has it. Cleared once added. */
  numberTitle?: boolean
  /**
   * Opened from the launcher by typing a host and not connected yet: its
   * first connection is remembered as recent and offered for saving.
   */
  quickConnect?: boolean
  /** Quick connection: the method that got the user in, once known. */
  quickConnectAuth?: SshAuthUsed
  connection?: {
    type?: ConnectionType
    profileId?: string
    profileName?: string
    host?: string
    port?: number
    username?: string
    password?: string
    ignoreSavedPassword?: boolean
    rememberPassword?: boolean
    keepaliveIntervalSecs?: number
    keepaliveCountMax?: number
    serverMonitorVisible?: boolean
    authMethod?: ConnectionAuthMethod
    agentForward?: boolean
    privateKeyPath?: string
    privateKeyPassphrase?: string
    terminalShell?: TerminalShellType
    terminalShellCustomPath?: string
    terminalShellCustomArgs?: string
    /** Local terminal: last directory the shell reported; a restart resumes there. */
    cwd?: string
    /** Ordered jump host chain to tunnel through. */
    jumpHosts?: JumpHostConnection[]
    /** SSH: charset of the remote shell; missing means UTF-8. */
    encoding?: string
    /** SSH: theme whose terminal colors this tab uses; missing means the app theme. */
    terminalTheme?: string
    /** SSH: commands typed into the shell once it is ready, one per line. */
    loginScript?: string
    /** SSH: start the shell with tTerm's shell integration when the host supports it. */
    shellIntegration?: boolean
    /** Whether this tab's sessions are logged; missing follows the logging setting. */
    sessionLog?: SessionLogPolicy
  }
  remoteFile?: {
    sourceTabId: string
    profileId?: string
    profileName?: string
    connectionLabel?: string
    connectionKey?: string
    host?: string
    path: string
    fileName: string
    size: number
    modifiedAt?: number
  }
}

/** Singleton app pages that live in a tab but have no connection or session. */
export type PageTabType = Extract<TabType, "settings" | "tunnels" | "logs">

export function isPageTab(tab: Pick<Tab, "type"> | null | undefined): boolean {
  return tab?.type === "settings" || tab?.type === "tunnels" || tab?.type === "logs"
}

export interface TabContextMenuAction {
  label: string
  action: string
  icon?: string
  separator?: boolean
  disabled?: boolean
}
