export interface Tab {
  id: string
  title: string
  type: "terminal" | "ssh" | "sftp" | "serial"
  isActive: boolean
  isModified?: boolean
  icon?: string
  pid?: number
  connection?: {
    type?: "terminal" | "ssh" | "sftp" | "serial"
    host?: string
    port?: number
    username?: string
    reconnect?: boolean
    reconnectDelaySecs?: number
    reconnectMaxDelaySecs?: number
    reconnectMaxRetries?: number
    keepaliveIntervalSecs?: number
    keepaliveCountMax?: number
  }
}

export interface TabContextMenuAction {
  label: string
  action: string
  icon?: string
  separator?: boolean
  disabled?: boolean
}
