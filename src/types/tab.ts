export interface Tab {
  id: string
  title: string
  type: "terminal" | "ssh" | "sftp" | "serial"
  isActive: boolean
  isModified?: boolean
  icon?: string
  connection?: {
    host?: string
    port?: number
    username?: string
  }
}

export interface TabContextMenuAction {
  label: string
  action: string
  icon?: string
  separator?: boolean
  disabled?: boolean
}
