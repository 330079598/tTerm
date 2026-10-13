/**
 * How a proxy carries the connection: an HTTP or SOCKS5 proxy server, or a
 * local command whose input and output carry it (OpenSSH's `ProxyCommand`).
 */
export type ProxyKind = "http" | "socks5" | "command"

/** The kinds of proxy that are servers with a host and port. */
export type NetworkProxyKind = Exclude<ProxyKind, "command">

/**
 * A saved proxy, without its password. A command proxy has only `command`;
 * the backend stores an empty host and port 0 for it.
 */
export interface ProxyServer {
  id: string
  name: string
  kind: ProxyKind
  host: string
  port: number
  /** Sign in to the proxy as this user; no authentication when missing. */
  username?: string
  /** The command line, with OpenSSH's `%h`, `%p`, `%r` and `%%` tokens. */
  command?: string
}

/** A saved proxy server as `list_proxies` returns it. */
export interface ProxyServerEntry extends ProxyServer {
  /** Whether a password is saved for the server's user. */
  hasPassword: boolean
}

/**
 * Which proxy a host connects through: this device's default proxy, none,
 * or a saved server by id.
 */
export type ProxyChoice = { mode: "default" } | { mode: "direct" } | { mode: "server"; id: string }
