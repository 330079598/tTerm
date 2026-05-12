import {
  ConnectionState,
  SshConnectionProgress,
  TerminalTabProps,
} from "@/components/TerminalTab/types"

export const FALLBACK_TERMINAL_BACKGROUND = "#111827"
export const TAB_ACTIVATE_REFIT_DELAY_MS = 32
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
    case "disconnected":
      return t("sessionHeader.disconnected", { defaultValue: "Disconnected" })
    case "error":
      return t("sessionHeader.error", { defaultValue: "Error" })
  }
}

export function getSshConnectionProgressLabel(progress: SshConnectionProgress): string {
  if (progress.message) {
    return progress.message
  }

  const address = progress.host ? `${progress.host}${progress.port ? `:${progress.port}` : ""}` : ""
  const hop = progress.hopIndex
    ? `jump host #${progress.hopIndex}${progress.totalHops ? `/${progress.totalHops}` : ""}`
    : "jump host"

  switch (progress.phase) {
    case "resolving_credentials":
      return "Resolving saved credentials"
    case "jump_connecting":
      return `Connecting to ${hop}${address ? ` ${address}` : ""}`
    case "jump_host_key_checking":
      return `Checking ${hop} fingerprint${address ? ` for ${address}` : ""}`
    case "jump_authenticating":
      return `Authenticating ${hop}${progress.username ? ` as ${progress.username}` : ""}`
    case "jump_connected":
      return `${hop} connected`
    case "tunnel_opening":
      return `Opening tunnel${address ? ` to ${address}` : ""}`
    case "target_connecting":
      return `Connecting to target${address ? ` ${address}` : ""}`
    case "target_host_key_checking":
      return `Checking target fingerprint${address ? ` for ${address}` : ""}`
    case "target_authenticating":
      return `Authenticating target${progress.username ? ` as ${progress.username}` : ""}`
    case "ready":
      return "Connection ready"
    default:
      return progress.phase.replace(/_/g, " ")
  }
}
