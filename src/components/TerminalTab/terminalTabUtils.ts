import {
  ConnectionState,
  SshConnectionProgress,
  TerminalTabProps,
} from "@/components/TerminalTab/types"

export const FALLBACK_TERMINAL_BACKGROUND = "#111827"
export const TAB_ACTIVATE_REFIT_DELAY_MS = 32
export const STATUS_CONNECTING = "[Connecting"

type Translator = (key: string, options?: Record<string, unknown>) => string

export function getConnectionDisplay(
  connection: TerminalTabProps["connection"] | undefined,
  t: Translator
): string {
  if (!connection || connection.type === "terminal") {
    return t("sessionHeader.localShell", { defaultValue: "Local shell" })
  }

  const host = connection.host || t("sessionHeader.unknownHost", { defaultValue: "unknown-host" })
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

export function getSshConnectionProgressLabel(
  progress: SshConnectionProgress,
  t: Translator
): string {
  // Retry phases render from structured fields so the status is localized;
  // every other phase still prefers the backend-provided message.
  if (progress.phase === "retrying") {
    return t("sessionHeader.reconnectRetryStatus", {
      reason: progress.reason ?? "",
      delay: Math.max(1, Math.round(progress.retryDelaySecs ?? 0)),
      attempt: progress.retryAttempt ?? 1,
      max: progress.retryMaxAttempts ?? 0,
      defaultValue:
        "Disconnected ({{reason}}). Reconnecting in {{delay}}s (attempt {{attempt}}/{{max}})",
    })
  }
  if (progress.phase === "retry_exhausted") {
    return t("sessionHeader.reconnectExhausted", {
      count: progress.retryMaxAttempts ?? 0,
      reason: progress.reason ?? "",
      defaultValue: "Automatic reconnect failed after {{count}} attempts: {{reason}}",
    })
  }

  if (progress.message) {
    return progress.message
  }

  const address = progress.host ? `${progress.host}${progress.port ? `:${progress.port}` : ""}` : ""
  const hopSuffix = address ? ` ${address}` : ""
  const forAddress = address ? ` for ${address}` : ""
  const asUsername = progress.username ? ` as ${progress.username}` : ""
  const hop = progress.hopIndex
    ? t("sessionHeader.jumpHostIndexed", {
        index: progress.hopIndex,
        totalSuffix: progress.totalHops ? `/${progress.totalHops}` : "",
        defaultValue: "jump host #{{index}}{{totalSuffix}}",
      })
    : t("sessionHeader.jumpHostFallback", { defaultValue: "jump host" })

  switch (progress.phase) {
    case "resolving_credentials":
      return t("sessionHeader.resolvingCredentials", {
        defaultValue: "Resolving saved credentials",
      })
    case "jump_connecting":
      return t("sessionHeader.jumpConnecting", {
        hop,
        address: hopSuffix,
        defaultValue: "Connecting to {{hop}}{{address}}",
      })
    case "jump_host_key_checking":
      return t("sessionHeader.jumpHostKeyChecking", {
        hop,
        forAddress,
        defaultValue: "Checking {{hop}} fingerprint{{forAddress}}",
      })
    case "jump_authenticating":
      return t("sessionHeader.jumpAuthenticating", {
        hop,
        asUsername,
        defaultValue: "Authenticating {{hop}}{{asUsername}}",
      })
    case "jump_connected":
      return t("sessionHeader.jumpConnected", { hop, defaultValue: "{{hop}} connected" })
    case "tunnel_opening":
      return t("sessionHeader.tunnelOpening", {
        toAddress: address ? ` to ${address}` : "",
        defaultValue: "Opening tunnel{{toAddress}}",
      })
    case "target_connecting":
      return t("sessionHeader.targetConnecting", {
        address: hopSuffix,
        defaultValue: "Connecting to target{{address}}",
      })
    case "target_host_key_checking":
      return t("sessionHeader.targetHostKeyChecking", {
        forAddress,
        defaultValue: "Checking target fingerprint{{forAddress}}",
      })
    case "target_authenticating":
      return t("sessionHeader.targetAuthenticating", {
        asUsername,
        defaultValue: "Authenticating target{{asUsername}}",
      })
    case "ready":
      return t("sessionHeader.connectionReady", { defaultValue: "Connection ready" })
    default:
      return progress.phase.replace(/_/g, " ")
  }
}
