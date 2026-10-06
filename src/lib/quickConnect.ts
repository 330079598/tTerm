import type {
  JumpHostConnection,
  SavedJumpHost,
  SavedProfile,
  SshAuthMethod,
  SshAuthUsed,
  Tab,
} from "@/types/tab"

/** One `[user@]host[:port]` endpoint, as typed. */
export interface QuickConnectEndpoint {
  username?: string
  host: string
  port?: number
}

/** What was typed into the new tab launcher, when it reads as a host to connect to. */
export interface QuickConnectTarget extends QuickConnectEndpoint {
  /** `-J`: the jump host to go through. */
  jumpHost?: QuickConnectEndpoint
  /** `-i`: the key file to try first. */
  privateKeyPath?: string
}

/** A `Host` entry of `~/.ssh/config`, as `preview_ssh_config_import` lists it. */
export interface SshConfigHost {
  hostPattern: string
  name: string
  host?: string
  port: number
  username?: string
  privateKeyPath?: string
  agentForward: boolean
  keepaliveIntervalSecs: number
  keepaliveCountMax: number
  jumpHosts: SavedJumpHost[]
  skipped: boolean
}

/** A connection to open: everything a quick connect tab needs. */
export interface QuickConnection {
  title: string
  username: string
  host: string
  port: number
  privateKeyPath?: string
  agentForward?: boolean
  keepaliveIntervalSecs?: number
  keepaliveCountMax?: number
  jumpHosts?: JumpHostConnection[]
}

/** A quick connection that reached a shell, offered again in the launcher. */
export interface RecentQuickConnection extends QuickConnection {
  lastUsedAt: number
}

export const QUICK_CONNECT_RECENTS_STORAGE_KEY = "tterm.quick-connect.recent.v1"
export const QUICK_CONNECT_RECENTS_LIMIT = 8
export const DEFAULT_SSH_PORT = 22

const HOSTNAME = /^[A-Za-z0-9_](?:[A-Za-z0-9._-]*[A-Za-z0-9_])?$/
const IPV6 = /^[0-9A-Fa-f:.%]+$/

function parsePort(value: string): number | null {
  if (!/^\d{1,5}$/.test(value)) return null
  const port = Number(value)
  return port >= 1 && port <= 65535 ? port : null
}

/** Reads `[user@]host[:port]`, with IPv6 addresses in brackets when a port follows. */
export function parseEndpoint(text: string): QuickConnectEndpoint | null {
  const at = text.lastIndexOf("@")
  const username = at >= 0 ? text.slice(0, at) : undefined
  const rest = at >= 0 ? text.slice(at + 1) : text
  if (username !== undefined && (!username || /[\s:@]/.test(username))) return null

  let host = rest
  let port: number | undefined
  const bracketed = /^\[([^\]]+)\](?::(\d+))?$/.exec(rest)
  if (bracketed) {
    if (!IPV6.test(bracketed[1])) return null
    host = bracketed[1]
    if (bracketed[2] !== undefined) {
      const parsed = parsePort(bracketed[2])
      if (parsed === null) return null
      port = parsed
    }
  } else if ((rest.match(/:/g) ?? []).length === 1) {
    const [name, portText] = rest.split(":")
    const parsed = parsePort(portText)
    if (parsed === null) return null
    host = name
    port = parsed
  } else if (rest.includes(":") && !IPV6.test(rest)) {
    return null
  }

  if (!host.includes(":") && !HOSTNAME.test(host)) return null
  return { username, host, port }
}

/**
 * Reads what was typed as a host to connect to: `[user@]host[:port]`, or an
 * `ssh` command line with `-p`, `-l`, `-i` and `-J`. Anything else (search
 * words, unsupported options, a remote command) is `null`.
 */
export function parseQuickConnectInput(input: string): QuickConnectTarget | null {
  const tokens = input.trim().split(/\s+/).filter(Boolean)
  if (tokens[0] === "ssh") tokens.shift()
  if (tokens.length === 0) return null

  let endpoint: QuickConnectEndpoint | null = null
  let port: number | undefined
  let username: string | undefined
  let jumpHost: QuickConnectEndpoint | undefined
  let privateKeyPath: string | undefined

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index]
    const option = /^-([pliJ])(.*)$/.exec(token)
    if (option) {
      const value = option[2] || tokens[++index]
      if (!value) return null
      switch (option[1]) {
        case "p": {
          const parsed = parsePort(value)
          if (parsed === null) return null
          port = parsed
          break
        }
        case "l":
          username = value
          break
        case "i":
          privateKeyPath = value
          break
        case "J": {
          // One hop only; a chain is set up in the connection dialog.
          if (value.includes(",")) return null
          const parsed = parseEndpoint(value)
          if (!parsed) return null
          jumpHost = parsed
          break
        }
      }
    } else if (token.startsWith("-") || endpoint) {
      return null
    } else {
      endpoint = parseEndpoint(token)
      if (!endpoint) return null
    }
  }

  if (!endpoint) return null
  return {
    ...endpoint,
    username: endpoint.username ?? username,
    port: port ?? endpoint.port,
    jumpHost,
    privateKeyPath,
  }
}

/** Shows an endpoint the way it is typed. */
export function formatEndpoint(endpoint: QuickConnectEndpoint): string {
  const host =
    endpoint.host.includes(":") && endpoint.port && endpoint.port !== DEFAULT_SSH_PORT
      ? `[${endpoint.host}]`
      : endpoint.host
  const user = endpoint.username ? `${endpoint.username}@` : ""
  const port = endpoint.port && endpoint.port !== DEFAULT_SSH_PORT ? `:${endpoint.port}` : ""
  return `${user}${host}${port}`
}

/** The `~/.ssh/config` entry a typed host names, if it is an alias there. */
export function findSshConfigHost(
  hosts: readonly SshConfigHost[],
  alias: string
): SshConfigHost | undefined {
  return hosts.find((host) => !host.skipped && host.hostPattern === alias)
}

function jumpHostsFromConfig(jumpHosts: readonly SavedJumpHost[]): JumpHostConnection[] {
  return jumpHosts.map((jump) => ({
    host: jump.host,
    port: jump.port,
    username: jump.username,
    authMethod: "auto",
    privateKeyPath: jump.private_key_path,
  }))
}

/**
 * The connection a typed target asks for, filled in from `~/.ssh/config`
 * when its host is an alias there, as `ssh` does. `null` while the user
 * name is still unknown.
 */
export function resolveQuickConnectTarget(
  target: QuickConnectTarget,
  configHosts: readonly SshConfigHost[]
): QuickConnection | null {
  const config = findSshConfigHost(configHosts, target.host)
  const username = target.username ?? config?.username
  if (!username) return null

  const host = config?.host ?? target.host
  const port = target.port ?? config?.port ?? DEFAULT_SSH_PORT
  const jump = target.jumpHost
  const jumpUsername = jump ? (jump.username ?? username) : undefined
  return {
    title:
      config && !target.port
        ? `${username}@${config.name}`
        : formatEndpoint({ username, host: target.host, port }),
    username,
    host,
    port,
    privateKeyPath: target.privateKeyPath ?? config?.privateKeyPath,
    agentForward: config?.agentForward,
    keepaliveIntervalSecs: config?.keepaliveIntervalSecs,
    keepaliveCountMax: config?.keepaliveCountMax,
    jumpHosts:
      jump && jumpUsername
        ? [
            {
              host: jump.host,
              port: jump.port ?? DEFAULT_SSH_PORT,
              username: jumpUsername,
              authMethod: "auto",
            },
          ]
        : config && config.jumpHosts.length > 0
          ? jumpHostsFromConfig(config.jumpHosts)
          : undefined,
  }
}

/** The connection of an `~/.ssh/config` entry; `null` without a `User`. */
export function quickConnectionFromConfig(config: SshConfigHost): QuickConnection | null {
  return resolveQuickConnectTarget({ host: config.hostPattern }, [config])
}

/** A tab for a quick connection: no saved profile, and the method is found while connecting. */
export function buildQuickConnectTab(connection: QuickConnection): Omit<Tab, "id" | "isActive"> {
  return {
    title: connection.title,
    type: "ssh",
    isModified: false,
    quickConnect: true,
    connection: {
      type: "ssh",
      profileName: connection.title,
      host: connection.host,
      port: connection.port,
      username: connection.username,
      authMethod: "auto",
      privateKeyPath: connection.privateKeyPath,
      agentForward: connection.agentForward === true,
      keepaliveIntervalSecs: connection.keepaliveIntervalSecs,
      keepaliveCountMax: connection.keepaliveCountMax,
      jumpHosts: connection.jumpHosts,
    },
  }
}

/** The quick connection a tab was opened with, to offer again from the launcher. */
export function quickConnectionFromTab(tab: Tab): QuickConnection | null {
  const connection = tab.connection
  if (!connection?.host || !connection.username) return null
  return {
    title: tab.title,
    username: connection.username,
    host: connection.host,
    port: connection.port ?? DEFAULT_SSH_PORT,
    privateKeyPath: connection.privateKeyPath,
    agentForward: connection.agentForward,
    keepaliveIntervalSecs: connection.keepaliveIntervalSecs,
    keepaliveCountMax: connection.keepaliveCountMax,
    jumpHosts: connection.jumpHosts?.map((jump) => ({
      host: jump.host,
      port: jump.port,
      username: jump.username,
      authMethod: "auto",
      privateKeyPath: jump.privateKeyPath,
    })),
  }
}

function recentKey(connection: QuickConnection): string {
  const jumps = (connection.jumpHosts ?? [])
    .map((jump) => `${jump.username}@${jump.host}:${jump.port}`)
    .join(",")
  return `${connection.username}@${connection.host}:${connection.port}|${jumps}`
}

export function addRecentQuickConnection(
  recents: readonly RecentQuickConnection[],
  connection: QuickConnection,
  usedAt: number
): RecentQuickConnection[] {
  const key = recentKey(connection)
  return [
    { ...connection, lastUsedAt: usedAt },
    ...recents.filter((recent) => recentKey(recent) !== key),
  ].slice(0, QUICK_CONNECT_RECENTS_LIMIT)
}

export function removeRecentQuickConnection(
  recents: readonly RecentQuickConnection[],
  connection: QuickConnection
): RecentQuickConnection[] {
  const key = recentKey(connection)
  return recents.filter((recent) => recentKey(recent) !== key)
}

export function loadRecentQuickConnections(
  storage: Pick<Storage, "getItem">
): RecentQuickConnection[] {
  try {
    const value = storage.getItem(QUICK_CONNECT_RECENTS_STORAGE_KEY)
    if (!value) return []
    const parsed: unknown = JSON.parse(value)
    if (!Array.isArray(parsed)) return []
    return parsed
      .filter(
        (item): item is RecentQuickConnection =>
          item !== null &&
          typeof item === "object" &&
          typeof item.title === "string" &&
          typeof item.username === "string" &&
          typeof item.host === "string" &&
          typeof item.port === "number" &&
          typeof item.lastUsedAt === "number"
      )
      .slice(0, QUICK_CONNECT_RECENTS_LIMIT)
  } catch {
    return []
  }
}

export function saveRecentQuickConnections(
  storage: Pick<Storage, "setItem">,
  recents: readonly RecentQuickConnection[]
) {
  try {
    storage.setItem(QUICK_CONNECT_RECENTS_STORAGE_KEY, JSON.stringify(recents))
  } catch {
    // Recents are a convenience; a full or blocked storage just forgets them.
  }
}

/** Whether every word of `query` appears in one of `fields`, ignoring case. */
export function matchesQuery(query: string, fields: ReadonlyArray<string | undefined>): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean)
  const haystack = fields
    .filter((field): field is string => Boolean(field))
    .join(" ")
    .toLowerCase()
  return words.every((word) => haystack.includes(word))
}

/**
 * Records what got a quick connection in. A method that needs nothing typed
 * replaces `auto` on the tab, so its SFTP, monitoring and reconnects go
 * straight to it.
 */
export function applyQuickConnectAuth(tab: Tab, auth: SshAuthUsed): Tab {
  if (tab.connection?.authMethod !== "auto") return tab
  return {
    ...tab,
    quickConnectAuth: auth,
    connection: auth.reusable
      ? {
          ...tab.connection,
          authMethod: auth.method,
          privateKeyPath: auth.method === "key" ? auth.privateKeyPath : undefined,
        }
      : tab.connection,
  }
}

/** The connection dialog's keepalive defaults. */
const DEFAULT_KEEPALIVE_INTERVAL_SECS = 15
const DEFAULT_KEEPALIVE_COUNT_MAX = 3

function draftAuthMethod(privateKeyPath: string | undefined): SshAuthMethod {
  return privateKeyPath ? "key" : "password"
}

/** A profile to save for a quick connection tab, with the method that worked. */
export function draftProfileFromTab(tab: Tab): SavedProfile {
  const connection = tab.connection ?? {}
  const auth = tab.quickConnectAuth
  const authMethod =
    connection.authMethod && connection.authMethod !== "auto"
      ? connection.authMethod
      : (auth?.method ?? draftAuthMethod(connection.privateKeyPath))
  const privateKeyPath = auth?.privateKeyPath ?? connection.privateKeyPath
  const jumpHosts = connection.jumpHosts ?? []
  return {
    id: "",
    name: tab.title,
    group: "",
    connection_type: "ssh",
    host: connection.host,
    port: connection.port ?? DEFAULT_SSH_PORT,
    username: connection.username,
    remember_password: false,
    auth_method: authMethod,
    agent_forward: connection.agentForward === true,
    private_key_path: authMethod === "key" ? privateKeyPath : undefined,
    keepalive_interval_secs: connection.keepaliveIntervalSecs ?? DEFAULT_KEEPALIVE_INTERVAL_SECS,
    keepalive_count_max: connection.keepaliveCountMax ?? DEFAULT_KEEPALIVE_COUNT_MAX,
    use_jump_host: jumpHosts.length > 0,
    jump_hosts: jumpHosts.map((jump) => ({
      host: jump.host,
      port: jump.port,
      username: jump.username,
      auth_method:
        jump.authMethod === "auto" ? draftAuthMethod(jump.privateKeyPath) : jump.authMethod,
      private_key_path: jump.privateKeyPath,
    })),
  }
}

/** A profile prefilled with what was typed, for the full connection dialog. */
export function draftProfileFromInput(
  input: string,
  configHosts: readonly SshConfigHost[]
): SavedProfile | null {
  const target = parseQuickConnectInput(input)
  if (!target) return null
  const connection = resolveQuickConnectTarget(target, configHosts) ?? {
    title: formatEndpoint(target),
    username: "",
    host: findSshConfigHost(configHosts, target.host)?.host ?? target.host,
    port: target.port ?? DEFAULT_SSH_PORT,
  }
  return draftProfileFromTab({
    id: "",
    isActive: false,
    ...buildQuickConnectTab(connection),
  })
}
