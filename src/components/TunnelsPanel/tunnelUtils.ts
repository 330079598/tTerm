import type { SavedProfile } from "@/types/tab"
import type {
  CredentialRequest,
  TunnelCredentials,
  TunnelKind,
  TunnelRule,
  TunnelState,
  TunnelStatus,
} from "@/types/tunnel"

export const TUNNEL_KINDS: TunnelKind[] = ["local", "remote", "dynamic"]

export type RouteNodeKind = "device" | "server" | "target"

export interface RouteNode {
  kind: RouteNodeKind
  /** i18n key suffix for fixed labels; `title` is used when present. */
  title?: string
  detail?: string
}

export function createTunnelId(): string {
  return `tunnel-${crypto.randomUUID()}`
}

export function createEmptyTunnel(kind: TunnelKind = "local"): TunnelRule {
  return {
    id: createTunnelId(),
    name: "",
    profileId: "",
    kind,
    bindHost: "127.0.0.1",
    bindPort: kind === "dynamic" ? 1080 : 0,
    destHost: kind === "dynamic" ? "" : "localhost",
    destPort: 0,
    autoStart: false,
  }
}

export function profileLabel(profile: SavedProfile | undefined): string {
  if (!profile) return ""
  return profile.name || `${profile.username ?? ""}@${profile.host ?? ""}`
}

function endpoint(host: string, port: number | string): string {
  const value = host.trim() || "…"
  const rendered = host.includes(":") && !host.startsWith("[") ? `[${value}]` : value
  return `${rendered}:${port || "…"}`
}

/**
 * The three hops a tunnel's traffic takes, in the order data flows from the
 * side that accepts connections to the side that receives them.
 */
export function getRouteNodes(
  rule: Pick<TunnelRule, "kind" | "bindHost" | "bindPort" | "destHost" | "destPort">,
  hostLabel: string
): [RouteNode, RouteNode, RouteNode] {
  const bind = endpoint(rule.bindHost, rule.bindPort)
  const dest = endpoint(rule.destHost, rule.destPort)

  switch (rule.kind) {
    case "local":
      return [
        { kind: "device", detail: bind },
        { kind: "server", title: hostLabel },
        { kind: "target", detail: dest },
      ]
    case "remote":
      return [
        { kind: "server", title: hostLabel, detail: bind },
        { kind: "device" },
        { kind: "target", detail: dest },
      ]
    case "dynamic":
      return [
        { kind: "device", detail: bind },
        { kind: "server", title: hostLabel },
        { kind: "target" },
      ]
  }
}

/** The equivalent OpenSSH invocation, shown as a hint in the editor. */
export function buildSshCommand(
  rule: Pick<TunnelRule, "kind" | "bindHost" | "bindPort" | "destHost" | "destPort">,
  profile: SavedProfile | undefined
): string {
  const target = profile?.host
    ? `${profile.username ? `${profile.username}@` : ""}${profile.host}`
    : "user@host"
  const port = profile?.port && profile.port !== 22 ? ` -p ${profile.port}` : ""
  const bind = `${rule.bindHost.trim() || "127.0.0.1"}:${rule.bindPort || "…"}`
  const dest = `${rule.destHost.trim() || "…"}:${rule.destPort || "…"}`

  switch (rule.kind) {
    case "local":
      return `ssh -N -L ${bind}:${dest}${port} ${target}`
    case "remote":
      return `ssh -N -R ${bind}:${dest}${port} ${target}`
    case "dynamic":
      return `ssh -N -D ${bind}${port} ${target}`
  }
}

export type TunnelFormError =
  | "nameRequired"
  | "hostRequired"
  | "bindHostRequired"
  | "bindPortInvalid"
  | "destHostRequired"
  | "destPortInvalid"

const isPort = (value: number) => Number.isInteger(value) && value >= 1 && value <= 65535

export function validateTunnel(rule: TunnelRule): TunnelFormError | null {
  if (!rule.name.trim()) return "nameRequired"
  if (!rule.profileId) return "hostRequired"
  if (!rule.bindHost.trim()) return "bindHostRequired"
  if (!isPort(rule.bindPort)) return "bindPortInvalid"
  if (rule.kind !== "dynamic") {
    if (!rule.destHost.trim()) return "destHostRequired"
    if (!isPort(rule.destPort)) return "destPortInvalid"
  }
  return null
}

/** Parses a port text field; blank or non-numeric input becomes 0 (invalid). */
export function parsePortInput(value: string): number {
  const trimmed = value.trim()
  if (!/^\d+$/.test(trimmed)) return 0
  return Number(trimmed)
}

export function suggestTunnelName(
  rule: Pick<TunnelRule, "kind" | "bindPort" | "destHost" | "destPort">,
  hostLabel: string
): string {
  if (!hostLabel) return ""
  switch (rule.kind) {
    case "local":
    case "remote":
      return rule.destHost && rule.destPort
        ? `${hostLabel} · ${rule.destHost}:${rule.destPort}`
        : hostLabel
    case "dynamic":
      return `${hostLabel} · SOCKS ${rule.bindPort || ""}`.trim()
  }
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B"
  const units = ["B", "KB", "MB", "GB", "TB"]
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  const value = bytes / 1024 ** exponent
  return `${exponent === 0 || value >= 100 ? Math.round(value) : value.toFixed(1)} ${units[exponent]}`
}

export function formatUptime(connectedAt: number | null, now: number): string {
  if (connectedAt == null) return ""
  const seconds = Math.max(0, Math.floor((now - connectedAt) / 1000))
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ${seconds % 60}s`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ${minutes % 60}m`
  return `${Math.floor(hours / 24)}d ${hours % 24}h`
}

export function isTunnelActive(state: TunnelState): boolean {
  return state !== "stopped" && state !== "error" && state !== "needsCredentials"
}

export function stoppedStatus(id: string): TunnelStatus {
  return {
    id,
    state: "stopped",
    message: null,
    boundPort: null,
    activeConnections: 0,
    totalConnections: 0,
    bytesUp: 0,
    bytesDown: 0,
    connectedAt: null,
    retryAttempt: 0,
  }
}

export function credentialFieldKey(request: Pick<CredentialRequest, "hop" | "kind">): string {
  return `${request.hop ?? "target"}:${request.kind}`
}

/**
 * Folds one round of answers into what has been entered so far. Later rounds
 * only re-ask for what was wrong, so earlier answers must be carried along.
 */
export function mergeCredentials(
  previous: TunnelCredentials,
  requests: CredentialRequest[],
  values: Record<string, string>,
  remember: boolean
): TunnelCredentials {
  const next: TunnelCredentials = {
    ...previous,
    jumpHosts: (previous.jumpHosts ?? []).map((entry) => ({ ...entry })),
    remember: previous.remember || remember,
  }
  for (const request of requests) {
    const value = values[credentialFieldKey(request)] ?? ""
    if (request.hop === null) {
      if (request.kind === "password") next.password = value
      else next.keyPassphrase = value
      continue
    }
    let entry = next.jumpHosts!.find((candidate) => candidate.index === request.hop)
    if (!entry) {
      entry = { index: request.hop }
      next.jumpHosts!.push(entry)
    }
    if (request.kind === "password") entry.password = value
    else entry.keyPassphrase = value
  }
  return next
}
