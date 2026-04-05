export interface SavedProfile {
  id: string
  name: string
  connection_type: string
  host?: string
  port?: number
  username?: string
  remember_password: boolean
  auth_method?: string
  private_key_path?: string
  reconnect: boolean
  reconnect_delay_secs: number
  reconnect_max_delay_secs: number
  reconnect_max_retries: number
  keepalive_interval_secs: number
  keepalive_count_max: number
}

export type TerminalShellType = "auto" | "cmd" | "powershell" | "pwsh" | "custom"

export interface Tab {
  id: string
  title: string
  type: "terminal" | "ssh"
  isActive: boolean
  isModified?: boolean
  icon?: string
  pid?: number
  connection?: {
    type?: "terminal" | "ssh"
    profileName?: string
    host?: string
    port?: number
    username?: string
    password?: string
    rememberPassword?: boolean
    reconnect?: boolean
    reconnectDelaySecs?: number
    reconnectMaxDelaySecs?: number
    reconnectMaxRetries?: number
    keepaliveIntervalSecs?: number
    keepaliveCountMax?: number
    privateKeyPath?: string
    privateKeyPassphrase?: string
    terminalShell?: TerminalShellType
    terminalShellCustomPath?: string
    terminalShellCustomArgs?: string
  }
}

export interface TabContextMenuAction {
  label: string
  action: string
  icon?: string
  separator?: boolean
  disabled?: boolean
}
