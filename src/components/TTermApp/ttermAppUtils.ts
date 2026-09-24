import { Tab } from "@/types/tab"
import type { ConnectionState } from "@/components/TerminalTab/types"

const DEFAULT_CONNECTION_HEADER_PINNED = true

type TerminalSessionTarget = {
  tabId: string
  sessionNonce: number
}

export function isTerminalConnectionUnavailable(
  connectionState: ConnectionState | null | undefined
): boolean {
  return (
    connectionState === "disconnected" ||
    connectionState === "reconnecting" ||
    connectionState === "error"
  )
}

/** A terminal waiting at a password prompt that a saved password can answer. */
export type SavedPasswordPromptTarget = TerminalSessionTarget & {
  /** The prompt text; the backend only writes while it is still waiting. */
  prompt: string
}

export function resolveSavedPasswordInjectionTargets(
  source: SavedPasswordPromptTarget,
  liveInputActive: boolean,
  broadcastTargets: TerminalSessionTarget[],
  savedPasswordPrompts: ReadonlyMap<string, { sessionNonce: number; prompt: string }>
): SavedPasswordPromptTarget[] {
  if (!liveInputActive) return [source]

  const linked: SavedPasswordPromptTarget[] = []
  for (const target of broadcastTargets) {
    if (target.tabId === source.tabId) continue
    const waiting = savedPasswordPrompts.get(target.tabId)
    if (waiting?.sessionNonce === target.sessionNonce) {
      linked.push({ ...target, prompt: waiting.prompt })
    }
  }
  return [source, ...linked]
}

export function buildTabFromConnection(
  connection: Omit<Tab, "id" | "isActive">
): Omit<Tab, "id" | "isActive"> {
  return {
    ...connection,
    sessionNonce: connection.sessionNonce ?? 0,
    connectionHeaderPinned: connection.connectionHeaderPinned ?? DEFAULT_CONNECTION_HEADER_PINNED,
  }
}
