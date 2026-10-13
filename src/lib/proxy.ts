import type { NetworkProxyKind, ProxyChoice, ProxyServer } from "@/types/proxy"

export const DEFAULT_PROXY_CHOICE: ProxyChoice = { mode: "default" }

/** The ports proxies of each kind usually listen on. */
export const DEFAULT_PROXY_PORTS: Record<NetworkProxyKind, number> = {
  http: 8080,
  socks5: 1080,
}

/** A stored proxy choice; anything unknown follows the default proxy. */
export function normalizeProxyChoice(value: unknown): ProxyChoice {
  if (typeof value !== "object" || value === null) return DEFAULT_PROXY_CHOICE
  const choice = value as { mode?: unknown; id?: unknown }
  if (choice.mode === "direct") return { mode: "direct" }
  if (choice.mode === "server" && typeof choice.id === "string" && choice.id) {
    return { mode: "server", id: choice.id }
  }
  return DEFAULT_PROXY_CHOICE
}

/** The choice to store or send: omitted when it follows the default proxy. */
export function proxyChoicePayload(choice: ProxyChoice): ProxyChoice | undefined {
  return choice.mode === "default" ? undefined : choice
}

/** A choice as the value of a `<select>` option. */
export function proxyChoiceKey(choice: ProxyChoice): string {
  return choice.mode === "server" ? `server:${choice.id}` : choice.mode
}

/** The choice a `<select>` option's value stands for. */
export function parseProxyChoiceKey(key: string): ProxyChoice {
  if (key === "direct") return { mode: "direct" }
  if (key.startsWith("server:") && key.length > "server:".length) {
    return { mode: "server", id: key.slice("server:".length) }
  }
  return DEFAULT_PROXY_CHOICE
}

/** `host:port`, with an IPv6 address in brackets. */
export function proxyAddress(server: Pick<ProxyServer, "host" | "port">): string {
  const host = server.host.includes(":") ? `[${server.host}]` : server.host
  return `${host}:${server.port}`
}

/** How a list names a server's protocol. */
export function proxyKindLabel(kind: NetworkProxyKind): string {
  return kind === "http" ? "HTTP" : "SOCKS5"
}

/** What a list shows under a proxy's name: its protocol and address, or its command. */
export function proxySummary(server: ProxyServer): string {
  if (server.kind === "command") return `$ ${server.command ?? ""}`
  return `${proxyKindLabel(server.kind)} ${proxyAddress(server)}`
}

/** Whether every `%` in a proxy command starts a token tTerm replaces. */
export function hasOnlyKnownProxyTokens(command: string): boolean {
  return /^(?:[^%]|%[%hnpr])*$/.test(command)
}

export type ProxyDraftError = "name" | "host" | "port" | "command" | "commandToken"

/** The first field of a proxy being edited that cannot be saved. */
export function validateProxyDraft(
  draft: Pick<ProxyServer, "name" | "kind" | "host" | "port" | "command">
): ProxyDraftError | null {
  if (!draft.name.trim()) return "name"
  if (draft.kind === "command") {
    const command = draft.command?.trim() ?? ""
    if (!command) return "command"
    return hasOnlyKnownProxyTokens(command) ? null : "commandToken"
  }
  const host = draft.host.trim()
  if (!host || /\s/.test(host)) return "host"
  if (!Number.isInteger(draft.port) || draft.port < 1 || draft.port > 65535) return "port"
  return null
}

/** How many of `profiles` connect through the server with `id`. */
export function countProfilesUsingProxy(profiles: Array<{ proxy?: unknown }>, id: string): number {
  return profiles.filter((profile) => {
    const choice = normalizeProxyChoice(profile.proxy)
    return choice.mode === "server" && choice.id === id
  }).length
}
