import { Tab } from "@/types/tab"

const DEFAULT_CONNECTION_HEADER_PINNED = true

export function buildTabFromConnection(
  connection: Omit<Tab, "id" | "isActive">
): Omit<Tab, "id" | "isActive"> {
  return {
    ...connection,
    sessionNonce: connection.sessionNonce ?? 0,
    connectionHeaderPinned: connection.connectionHeaderPinned ?? DEFAULT_CONNECTION_HEADER_PINNED,
  }
}
