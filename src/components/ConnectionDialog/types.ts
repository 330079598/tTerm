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

export interface JumpHostForm {
  id: string
  host: string
  port: number
  username: string
  authMethod: "password" | "key"
  password: string
  privateKeyPath: string
  privateKeyPassphrase: string
}

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
  keepaliveIntervalSecs: number
  keepaliveCountMax: number
  terminalShell: TerminalShellType
  terminalShellCustomPath: string
  terminalShellCustomArgs: string
  // Jump host chain fields
  useJumpHost: boolean
  jumpHosts: JumpHostForm[]
}

export const createDefaultJumpHost = (): JumpHostForm => ({
  id: crypto.randomUUID(),
  host: "",
  port: 22,
  username: "",
  authMethod: "password",
  password: "",
  privateKeyPath: "",
  privateKeyPassphrase: "",
})

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
  keepaliveIntervalSecs: 15,
  keepaliveCountMax: 3,
  terminalShell: "auto",
  terminalShellCustomPath: "",
  terminalShellCustomArgs: "",
  useJumpHost: false,
  jumpHosts: [],
}

export const connectionTypes = [
  { type: "terminal" as const, label: "OS terminal" },
  { type: "ssh" as const, label: "SSH Connection" },
]
