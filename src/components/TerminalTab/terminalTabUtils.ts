import { ConnectionState, TerminalTabProps } from "@/components/TerminalTab/types"

export const FALLBACK_TERMINAL_BACKGROUND = "#111827"
export const TAB_ACTIVATE_REFIT_DELAY_MS = 32
export const STATUS_RECONNECT_PREFIX = "[SSH disconnected. Reconnect attempt"
export const STATUS_RECONNECTED = "[SSH reconnected]"
export const STATUS_CONNECTING = "[Connecting"

export function getConnectionDisplay(connection?: TerminalTabProps["connection"]): string {
  if (!connection || connection.type === "terminal") {
    return "Local shell"
  }

  const host = connection.host || "unknown-host"
  const port = connection.port ?? 22
  const address = `${host}:${port}`
  return connection.username ? `${connection.username}@${address}` : address
}

export function getConnectionStateLabel(
  state: ConnectionState,
  t: (key: string, options?: Record<string, unknown>) => string
): string {
  switch (state) {
    case "connecting":
      return t("sessionHeader.connecting", { defaultValue: "Connecting" })
    case "connected":
      return t("sessionHeader.connected", { defaultValue: "Connected" })
    case "reconnecting":
      return t("sessionHeader.reconnecting", { defaultValue: "Reconnecting" })
    case "disconnected":
      return t("sessionHeader.disconnected", { defaultValue: "Disconnected" })
    case "error":
      return t("sessionHeader.error", { defaultValue: "Error" })
  }
}
