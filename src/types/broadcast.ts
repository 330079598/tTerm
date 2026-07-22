import type { ConnectionState } from "@/components/TerminalTab/types"

export type BroadcastMode = "command" | "live"
export type LiveBroadcastState = "idle" | "active" | "paused"
export type TerminalInputKind = "keyboard" | "paste"

export type TerminalRuntimeState = {
  connectionState: ConnectionState
  sessionNonce: number
}

export type PtyWriteStatus = "written" | "stale" | "missing" | "reconnecting" | "failed"

export type PtyWriteResult = {
  tabId: string
  status: PtyWriteStatus
  error?: string
}

export type TerminalInputRequest = {
  data: string
  kind: TerminalInputKind
  sessionNonce: number
  tabId: string
}
