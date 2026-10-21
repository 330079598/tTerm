import { SavedProfile } from "@/components/ProfilesPanel"
import { useConfig } from "@/contexts/ConfigContext"
import type { DistroId } from "@/lib/distro"
import { DEFAULT_PROXY_CHOICE } from "@/lib/proxy"
import { DEFAULT_SERIAL_SETTINGS } from "@/lib/serial"
import type { SftpStartDirectory } from "@/lib/sftpStartDirectory"
import type { TabIconStyle } from "@/lib/tabIcons"
import type { ProxyChoice } from "@/types/proxy"
import type { SerialSettings } from "@/types/serial"
import { SessionLogPolicy, SshAuthMethod, Tab, TerminalShellType } from "@/types/tab"

export interface ConnectionDialogProps {
  isOpen: boolean
  onClose: () => void
  onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  editProfile?: SavedProfile | null
  duplicateProfile?: SavedProfile | null
  /** A new profile's starting values, e.g. from a quick connection. */
  draftProfile?: SavedProfile | null
  /** Only save the profile; nothing is connected. Editing a profile always only saves. */
  saveOnly?: boolean
  /** The quick connection tab being saved, whose typed passwords can be stored. */
  typedPasswordTabId?: string
  /** Offer to reconnect the tab the profile was edited from once it is saved. */
  canReconnect?: boolean
  onSaved?: (profile: SavedProfile, options: ProfileSavedOptions) => void
}

export interface ProfileSavedOptions {
  /** Reconnect the tab the dialog was opened from with the saved profile. */
  reconnect: boolean
}

export type ConnectionType = "terminal" | "ssh" | "serial"

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
  /** OpenSSH certificate for the key; empty uses `<key>-cert.pub` beside it. */
  certificatePath: string
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
  /** OpenSSH certificate for the key; empty uses `<key>-cert.pub` beside it. */
  certificatePath: string
  keepaliveIntervalSecs: number
  keepaliveCountMax: number
  encoding: TerminalEncoding
  /** Theme id for this connection's terminal colors; empty follows the app theme. */
  terminalTheme: string
  loginScript: string
  /** The icon the connection's tabs show; missing shows the host's distribution. */
  tabIcon?: TabIconStyle
  /** Read-only: the distribution the saved host was seen running. */
  distro?: DistroId
  /** Install tTerm's shell integration on the host so its shell marks commands. */
  shellIntegration: boolean
  /** Whether the connection's sessions are logged; `default` follows the logging setting. */
  sessionLog: SessionLogPolicy | "default"
  /** Where the connection's SFTP drawer opens; `default` follows the SFTP setting. */
  sftpStartDirectory: SftpStartDirectory | "default"
  /** The keyword highlight set's id or `off`; `default` follows the default set. */
  highlightSet: string
  terminalShell: TerminalShellType
  terminalShellCustomPath: string
  terminalShellCustomArgs: string
  // Jump host chain fields
  useJumpHost: boolean
  jumpHosts: JumpHostForm[]
  /** Which proxy the first host is reached through. */
  proxy: ProxyChoice
  // Sudo password autofill
  sudoAutofill: boolean
  sudoPassword: string
  clearSudoPassword: boolean
  /** The port and how it is opened, for a serial connection. */
  serial: SerialSettings
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
  certificatePath: "",
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
  certificatePath: "",
  keepaliveIntervalSecs: 15,
  keepaliveCountMax: 3,
  encoding: DEFAULT_TERMINAL_ENCODING,
  terminalTheme: "",
  loginScript: "",
  shellIntegration: false,
  sessionLog: "default",
  sftpStartDirectory: "default",
  highlightSet: "default",
  terminalShell: "auto",
  terminalShellCustomPath: "",
  terminalShellCustomArgs: "",
  useJumpHost: false,
  jumpHosts: [],
  proxy: DEFAULT_PROXY_CHOICE,
  sudoAutofill: true,
  sudoPassword: "",
  clearSudoPassword: false,
  serial: DEFAULT_SERIAL_SETTINGS,
}

export const KEEPALIVE_INTERVAL_RANGE = { min: 5, max: 3600, fallback: 15 } as const
export const KEEPALIVE_COUNT_RANGE = { min: 1, max: 100, fallback: 3 } as const

export const connectionTypes = [
  { type: "terminal" as const, label: "OS terminal" },
  { type: "ssh" as const, label: "SSH Connection" },
  { type: "serial" as const, label: "Serial port" },
]
