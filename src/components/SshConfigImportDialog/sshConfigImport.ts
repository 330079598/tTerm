import type { NetworkProxyKind } from "@/types/proxy"
import type { SavedJumpHost, SavedProfile } from "@/types/tab"
import type { ForwardSpec } from "@/types/tunnel"

/**
 * What a host's `ProxyCommand` sets up: no proxy, the HTTP or SOCKS5 server
 * the command only handed the connection to, or the command itself.
 */
export type SshConfigImportProxy =
  | { mode: "direct" }
  | { mode: "server"; kind: NetworkProxyKind; host: string; port: number }
  | { mode: "command"; command: string }

export interface SshConfigImportHost {
  hostPattern: string
  name: string
  host?: string
  port: number
  username?: string
  authMethod: string
  privateKeyPath?: string
  certificatePath?: string
  agentForward: boolean
  keepaliveIntervalSecs: number
  keepaliveCountMax: number
  jumpHosts: SavedJumpHost[]
  proxy?: SshConfigImportProxy
  warnings: string[]
  unsupportedOptions: string[]
  forwards: ForwardSpec[]
  skipped: boolean
  skipReason?: string
  existingProfileId?: string
}

export interface SshConfigImportPreview {
  sourcePath: string
  hosts: SshConfigImportHost[]
  warnings: string[]
}

export interface SshConfigImportResult {
  imported: number
  updated: number
  skipped: number
  tunnelsImported: number
  tunnelsSkipped: number
  profiles: SavedProfile[]
}

export const statusForHost = (host: SshConfigImportHost) => {
  if (host.skipped) return "skipped"
  if (host.unsupportedOptions.length > 0 || host.warnings.length > 0) return "warning"
  return "ready"
}
