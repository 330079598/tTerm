import type { SavedJumpHost, SavedProfile } from "@/types/tab"
import type { ForwardSpec } from "@/types/tunnel"

interface SshConfigImportHost {
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
