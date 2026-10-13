/** The protocol a proxy server speaks. */
export type ProxyKind = "http" | "socks5"

/** A saved proxy server, without its password. */
export interface ProxyServer {
  id: string
  name: string
  kind: ProxyKind
  host: string
  port: number
  /** Sign in to the proxy as this user; no authentication when missing. */
  username?: string
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
