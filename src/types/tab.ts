export interface SavedProfile {
  id: string
  name: string
  group: string
  connection_type: string
  host?: string
  port?: number
  username?: string
  password?: string
  remember_password: boolean
  auth_method?: string
  private_key_path?: string
  private_key_passphrase?: string
  keepalive_interval_secs: number
  keepalive_count_max: number
  jump_host?: SavedJumpHost
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

export type TerminalShellType = "auto" | "cmd" | "powershell" | "pwsh" | "custom"
export type ConnectionType = "terminal" | "ssh"
export type TabType = ConnectionType | "settings"

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
    rememberPassword?: boolean
    keepaliveIntervalSecs?: number
    keepaliveCountMax?: number
    privateKeyPath?: string
    privateKeyPassphrase?: string
    terminalShell?: TerminalShellType
    terminalShellCustomPath?: string
    terminalShellCustomArgs?: string
    /** Optional jump host (bastion) to tunnel through. */
    jumpHost?: JumpHostConnection
  }
}

export interface TabContextMenuAction {
  label: string
  action: string
  icon?: string
  separator?: boolean
  disabled?: boolean
}
