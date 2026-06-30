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
  private_key_path?: string
  private_key_passphrase?: string
  keepalive_interval_secs: number
  keepalive_count_max: number
  server_monitor_visible?: boolean
  jump_hosts?: SavedJumpHost[]
}

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
}

export type TerminalShellType =
  | "auto"
  | "cmd"
  | "powershell"
  | "pwsh"
  | "wsl"
  | "git-bash"
  | "custom"
export type ConnectionType = "terminal" | "ssh"
export type TabType = ConnectionType | "settings" | "remote-file-editor"

export interface JumpHostConnection {
  host: string
  port: number
  username: string
  authMethod: "password" | "key"
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
    privateKeyPath?: string
    privateKeyPassphrase?: string
    terminalShell?: TerminalShellType
    terminalShellCustomPath?: string
    terminalShellCustomArgs?: string
    /** Ordered jump host chain to tunnel through. */
    jumpHosts?: JumpHostConnection[]
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

export interface TabContextMenuAction {
  label: string
  action: string
  icon?: string
  separator?: boolean
  disabled?: boolean
}
