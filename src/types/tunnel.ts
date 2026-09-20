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
}

export type TunnelState = "stopped" | "starting" | "running" | "reconnecting" | "error"

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
