import { SavedProfile } from "@/components/ProfilesPanel"
import { useConfig } from "@/contexts/ConfigContext"
import { Tab, TerminalShellType } from "@/types/tab"

export interface ConnectionDialogProps {
  isOpen: boolean
  onClose: () => void
  onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  editProfile?: SavedProfile | null
}

export type ConnectionType = "terminal" | "ssh"

export type ConfigState = ReturnType<typeof useConfig>["config"]
export type SaveConfig = ReturnType<typeof useConfig>["saveConfig"]

export interface ConnectionForm {
  type: ConnectionType
  title: string
  group: string
  host: string
  port: number
  username: string
  authMethod: "password" | "key"
  password: string
  rememberPassword: boolean
  privateKeyPath: string
  privateKeyPassphrase: string
  reconnect: boolean
  reconnectDelaySecs: number
  reconnectMaxDelaySecs: number
  reconnectMaxRetries: number
  keepaliveIntervalSecs: number
  keepaliveCountMax: number
  terminalShell: TerminalShellType
  terminalShellCustomPath: string
  terminalShellCustomArgs: string
}

export interface ConnectionDialogContentProps extends Omit<ConnectionDialogProps, "isOpen"> {
  config: ConfigState
  saveConfig: SaveConfig
}

export const defaultForm: ConnectionForm = {
  type: "terminal",
  title: "",
  group: "",
  host: "",
  port: 22,
  username: "",
  authMethod: "password",
  password: "",
  rememberPassword: false,
  privateKeyPath: "",
  privateKeyPassphrase: "",
  reconnect: false,
  reconnectDelaySecs: 3,
  reconnectMaxDelaySecs: 60,
  reconnectMaxRetries: 8,
  keepaliveIntervalSecs: 15,
  keepaliveCountMax: 3,
  terminalShell: "auto",
  terminalShellCustomPath: "",
  terminalShellCustomArgs: "",
}

export const connectionTypes = [
  { type: "terminal" as const, label: "OS terminal" },
  { type: "ssh" as const, label: "SSH Connection" },
]
