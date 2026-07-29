import { Tab } from "@/types/tab"
import type { ConnectionState } from "@/components/TerminalTab/types"

const DEFAULT_CONNECTION_HEADER_PINNED = true

export function isTerminalConnectionUnavailable(
  connectionState: ConnectionState | null | undefined
): boolean {
  return connectionState === "disconnected" || connectionState === "error"
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
