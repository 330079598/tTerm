import { SavedProfile } from "@/components/ProfilesPanel"
import { useConfig } from "@/contexts/ConfigContext"
import { SshAuthMethod, Tab, TerminalShellType } from "@/types/tab"

export interface ConnectionDialogProps {
  isOpen: boolean
  onClose: () => void
  onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  editProfile?: SavedProfile | null
  duplicateProfile?: SavedProfile | null
}

export type ConnectionType = "terminal" | "ssh"

export type ConfigState = ReturnType<typeof useConfig>["config"]
export type SaveConfig = ReturnType<typeof useConfig>["saveConfig"]

/** Charsets offered for SSH sessions, as WHATWG labels the backend resolves. */
export const TERMINAL_ENCODINGS = [
  "utf-8",
  "gbk",
  "gb18030",
  "big5",
  "shift_jis",
  "euc-jp",
  "euc-kr",
  "windows-1252",
  "windows-1251",
  "koi8-r",
] as const

export type TerminalEncoding = (typeof TERMINAL_ENCODINGS)[number]

export const DEFAULT_TERMINAL_ENCODING: TerminalEncoding = "utf-8"

export interface JumpHostForm {
  id: string
  host: string
  port: number
  username: string
  authMethod: SshAuthMethod
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
  authMethod: SshAuthMethod
  agentForward: boolean
  password: string
  rememberPassword: boolean
  privateKeyPath: string
  privateKeyPassphrase: string
  keepaliveIntervalSecs: number
  keepaliveCountMax: number
  encoding: TerminalEncoding
  terminalShell: TerminalShellType
  terminalShellCustomPath: string
  terminalShellCustomArgs: string
  // Jump host chain fields
  useJumpHost: boolean
  jumpHosts: JumpHostForm[]
  // Sudo password autofill
  sudoAutofill: boolean
  sudoPassword: string
  clearSudoPassword: boolean
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
  agentForward: false,
  password: "",
  rememberPassword: false,
  privateKeyPath: "",
  privateKeyPassphrase: "",
  keepaliveIntervalSecs: 15,
  keepaliveCountMax: 3,
  encoding: DEFAULT_TERMINAL_ENCODING,
  terminalShell: "auto",
  terminalShellCustomPath: "",
  terminalShellCustomArgs: "",
  useJumpHost: false,
  jumpHosts: [],
  sudoAutofill: true,
  sudoPassword: "",
  clearSudoPassword: false,
}

export const KEEPALIVE_INTERVAL_RANGE = { min: 5, max: 3600, fallback: 15 } as const
export const KEEPALIVE_COUNT_RANGE = { min: 1, max: 100, fallback: 3 } as const

export const connectionTypes = [
  { type: "terminal" as const, label: "OS terminal" },
  { type: "ssh" as const, label: "SSH Connection" },
]
