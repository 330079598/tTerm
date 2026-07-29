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
  return connectionState === "disconnected" || connectionState === "error"
}

export function resolveSavedPasswordInjectionTargets(
  source: TerminalSessionTarget,
  liveInputActive: boolean,
  broadcastTargets: TerminalSessionTarget[],
  savedPasswordPrompts: ReadonlyMap<string, number>
): TerminalSessionTarget[] {
  if (!liveInputActive) return [source]

  return [
    source,
    ...broadcastTargets.filter(
      (target) =>
        target.tabId !== source.tabId &&
        savedPasswordPrompts.get(target.tabId) === target.sessionNonce
    ),
  ]
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
