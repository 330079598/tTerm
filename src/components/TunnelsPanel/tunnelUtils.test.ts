import { describe, expect, it } from "vitest"

import type { SavedProfile } from "@/types/tab"
import type { TunnelRule } from "@/types/tunnel"
import {
  buildSshCommand,
  formatBytes,
  formatUptime,
  getRouteNodes,
  isTunnelActive,
  parsePortInput,
  suggestTunnelName,
  validateTunnel,
} from "@/components/TunnelsPanel/tunnelUtils"

const profile = {
  id: "p1",
  name: "prod",
  connection_type: "ssh",
  host: "10.0.0.5",
  port: 2222,
  username: "deploy",
} as SavedProfile

function rule(overrides: Partial<TunnelRule> = {}): TunnelRule {
  return {
    id: "t1",
    name: "db",
    profileId: "p1",
    kind: "local",
    bindHost: "127.0.0.1",
    bindPort: 5432,
    destHost: "db.internal",
    destPort: 5432,
    ...overrides,
  }
}

describe("getRouteNodes", () => {
  it("routes a local forward from this device through the host to the target", () => {
    const [from, via, to] = getRouteNodes(rule(), "prod")
    expect(from).toEqual({ kind: "device", detail: "127.0.0.1:5432" })
    expect(via).toEqual({ kind: "server", title: "prod" })
    expect(to).toEqual({ kind: "target", detail: "db.internal:5432" })
  })

  it("starts a remote forward at the server's listening address", () => {
    const [from, via, to] = getRouteNodes(
      rule({
        kind: "remote",
        bindHost: "0.0.0.0",
        bindPort: 8080,
        destHost: "localhost",
        destPort: 3000,
      }),
      "prod"
    )
    expect(from).toEqual({ kind: "server", title: "prod", detail: "0.0.0.0:8080" })
    expect(via.kind).toBe("device")
    expect(to.detail).toBe("localhost:3000")
  })

  it("labels a dynamic forward as a SOCKS5 proxy with no fixed target", () => {
    const [from, , to] = getRouteNodes(rule({ kind: "dynamic", bindPort: 1080 }), "prod")
    expect(from.detail).toBe("127.0.0.1:1080")
    expect(to.detail).toBeUndefined()
  })

  it("brackets IPv6 hosts and shows placeholders for unfinished fields", () => {
    const [from, , to] = getRouteNodes(rule({ bindHost: "::1", destHost: "", destPort: 0 }), "prod")
    expect(from.detail).toBe("[::1]:5432")
    expect(to.detail).toBe("…:…")
  })
})

describe("buildSshCommand", () => {
  it("renders the equivalent OpenSSH flags for each kind", () => {
    expect(buildSshCommand(rule(), profile)).toBe(
      "ssh -N -L 127.0.0.1:5432:db.internal:5432 -p 2222 deploy@10.0.0.5"
    )
    expect(buildSshCommand(rule({ kind: "remote", bindPort: 8080 }), profile)).toBe(
      "ssh -N -R 127.0.0.1:8080:db.internal:5432 -p 2222 deploy@10.0.0.5"
    )
    expect(buildSshCommand(rule({ kind: "dynamic", bindPort: 1080 }), profile)).toBe(
      "ssh -N -D 127.0.0.1:1080 -p 2222 deploy@10.0.0.5"
    )
  })

  it("omits the default port and falls back when no host is chosen", () => {
    expect(buildSshCommand(rule(), { ...profile, port: 22 })).not.toContain("-p")
    expect(buildSshCommand(rule(), undefined)).toContain("user@host")
  })
})

describe("validateTunnel", () => {
  it("accepts a complete rule", () => {
    expect(validateTunnel(rule())).toBeNull()
  })

  it("reports the first missing field", () => {
    expect(validateTunnel(rule({ name: " " }))).toBe("nameRequired")
    expect(validateTunnel(rule({ profileId: "" }))).toBe("hostRequired")
    expect(validateTunnel(rule({ bindPort: 0 }))).toBe("bindPortInvalid")
    expect(validateTunnel(rule({ bindPort: 70000 }))).toBe("bindPortInvalid")
    expect(validateTunnel(rule({ destHost: "" }))).toBe("destHostRequired")
    expect(validateTunnel(rule({ destPort: 0 }))).toBe("destPortInvalid")
  })

  it("does not require a destination for dynamic forwards", () => {
    expect(validateTunnel(rule({ kind: "dynamic", destHost: "", destPort: 0 }))).toBeNull()
  })
})

describe("parsePortInput", () => {
  it("parses digits and rejects everything else as 0", () => {
    expect(parsePortInput(" 8080 ")).toBe(8080)
    expect(parsePortInput("")).toBe(0)
    expect(parsePortInput("80a")).toBe(0)
    expect(parsePortInput("-1")).toBe(0)
  })
})

describe("formatting", () => {
  it("formats byte counts", () => {
    expect(formatBytes(0)).toBe("0 B")
    expect(formatBytes(512)).toBe("512 B")
    expect(formatBytes(1536)).toBe("1.5 KB")
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB")
  })

  it("formats uptime", () => {
    expect(formatUptime(null, 1000)).toBe("")
    expect(formatUptime(0, 45_000)).toBe("45s")
    expect(formatUptime(0, 125_000)).toBe("2m 5s")
    expect(formatUptime(0, 3_720_000)).toBe("1h 2m")
    expect(formatUptime(0, 90_000_000)).toBe("1d 1h")
  })
})

describe("misc", () => {
  it("suggests a name from the host and destination", () => {
    expect(suggestTunnelName(rule(), "prod")).toBe("prod · db.internal:5432")
    expect(suggestTunnelName(rule({ kind: "dynamic", bindPort: 1080 }), "prod")).toBe(
      "prod · SOCKS 1080"
    )
    expect(suggestTunnelName(rule(), "")).toBe("")
  })

  it("treats stopped and error tunnels as inactive", () => {
    expect(isTunnelActive("running")).toBe(true)
    expect(isTunnelActive("reconnecting")).toBe(true)
    expect(isTunnelActive("stopped")).toBe(false)
    expect(isTunnelActive("error")).toBe(false)
  })
})
