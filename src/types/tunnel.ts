export type TunnelKind = "local" | "remote" | "dynamic"

export interface TunnelRule {
  id: string
  name: string
  profileId: string
  kind: TunnelKind
  bindHost: string
  bindPort: number
  destHost: string
  destPort: number
  /** Start when the app opens. */
  autoStart: boolean
}

export type TunnelState =
  | "stopped"
  | "starting"
  | "running"
  | "reconnecting"
  | "error"
  | "needsCredentials"

export interface TunnelStatus {
  id: string
  state: TunnelState
  message: string | null
  boundPort: number | null
  activeConnections: number
  totalConnections: number
  bytesUp: number
  bytesDown: number
  /** Unix ms when the current session came up. */
  connectedAt: number | null
  retryAttempt: number
}

/** A secret the backend needs before it can connect. */
export interface CredentialRequest {
  /** Jump host position, or null for the target host. */
  hop: number | null
  kind: "password" | "passphrase"
  /** `user@host:port` the secret is for. */
  label: string
  /** A passphrase was supplied earlier but did not unlock the key. */
  incorrect: boolean
}

export type StartOutcome =
  | { status: "started" }
  | { status: "needsCredentials"; requests: CredentialRequest[] }

export interface TunnelCredentials {
  password?: string
  keyPassphrase?: string
  /** Save entered passwords in secure storage. */
  remember?: boolean
  jumpHosts?: Array<{ index: number; password?: string; keyPassphrase?: string }>
}
