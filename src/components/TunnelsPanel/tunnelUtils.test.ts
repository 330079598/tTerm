import { describe, expect, it } from "vitest"

import type { SavedProfile } from "@/types/tab"
import type { TunnelRule } from "@/types/tunnel"
import {
  buildSshCommand,
  describeTransition,
  formatBytes,
  formatForward,
  formatUptime,
  getRouteNodes,
  isTunnelActive,
  mergeCredentials,
  credentialFieldKey,
  parsePortInput,
  summarizeTunnelNames,
  suggestTunnelName,
  tunnelBrowserUrl,
  tunnelClientAddress,
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
    autoStart: false,
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
    expect(isTunnelActive("needsCredentials")).toBe(false)
  })
})

describe("mergeCredentials", () => {
  const target = { hop: null, kind: "password", label: "a@b:22", incorrect: false } as const
  const jump = { hop: 1, kind: "passphrase", label: "o@j:22", incorrect: false } as const

  it("maps answers onto the target and the right jump host", () => {
    const merged = mergeCredentials(
      {},
      [target, jump],
      { [credentialFieldKey(target)]: "pw", [credentialFieldKey(jump)]: "pp" },
      true
    )
    expect(merged.password).toBe("pw")
    expect(merged.jumpHosts).toEqual([{ index: 1, keyPassphrase: "pp" }])
    expect(merged.remember).toBe(true)
  })

  it("keeps earlier answers when only one secret is asked again", () => {
    const first = mergeCredentials({}, [target], { [credentialFieldKey(target)]: "pw" }, false)
    const wrongKey = { hop: null, kind: "passphrase", label: "a@b:22", incorrect: true } as const
    const second = mergeCredentials(
      first,
      [wrongKey],
      { [credentialFieldKey(wrongKey)]: "pp" },
      false
    )
    expect(second.password).toBe("pw")
    expect(second.keyPassphrase).toBe("pp")
    expect(first.keyPassphrase).toBeUndefined()
  })

  it("does not drop a remember choice made in an earlier round", () => {
    const first = mergeCredentials({}, [target], { [credentialFieldKey(target)]: "pw" }, true)
    expect(mergeCredentials(first, [], {}, false).remember).toBe(true)
  })
})

describe("client addresses", () => {
  it("copies host:port for local forwards and a socks5h URL for dynamic ones", () => {
    expect(tunnelClientAddress(rule({ bindHost: "127.0.0.1", bindPort: 5432 }), null)).toBe(
      "127.0.0.1:5432"
    )
    expect(tunnelClientAddress(rule({ kind: "dynamic", bindPort: 1080 }), null)).toBe(
      "socks5h://127.0.0.1:1080"
    )
  })

  it("prefers the port actually bound and turns wildcards into loopback", () => {
    expect(tunnelClientAddress(rule({ bindHost: "0.0.0.0", bindPort: 5432 }), 6000)).toBe(
      "127.0.0.1:6000"
    )
    expect(tunnelClientAddress(rule({ bindHost: "::1", bindPort: 5432 }), null)).toBe("[::1]:5432")
  })

  it("has nothing to copy for remote forwards", () => {
    expect(tunnelClientAddress(rule({ kind: "remote" }), null)).toBeNull()
  })
})

describe("tunnelBrowserUrl", () => {
  it("opens local forwards to well-known web ports", () => {
    const web = rule({ bindHost: "127.0.0.1", bindPort: 9000, destPort: 8080 })
    expect(tunnelBrowserUrl(web, null)).toBe("http://127.0.0.1:9000")
    expect(tunnelBrowserUrl({ ...web, destPort: 443 }, null)).toBe("https://127.0.0.1:9000")
  })

  it("offers nothing for databases, SOCKS or remote forwards", () => {
    expect(tunnelBrowserUrl(rule({ destPort: 5432 }), null)).toBeNull()
    expect(tunnelBrowserUrl(rule({ kind: "dynamic", destPort: 0 }), null)).toBeNull()
    expect(tunnelBrowserUrl(rule({ kind: "remote", destPort: 8080 }), null)).toBeNull()
  })
})

describe("formatForward", () => {
  it("describes each kind on one line", () => {
    const base = { bindHost: "localhost", bindPort: 5432, destHost: "db", destPort: 5432 }
    expect(formatForward({ kind: "local", ...base })).toBe("L localhost:5432 → db:5432")
    expect(formatForward({ kind: "remote", ...base })).toBe("R localhost:5432 → db:5432")
    expect(formatForward({ kind: "dynamic", ...base, destHost: "", destPort: 0 })).toBe(
      "D localhost:5432"
    )
  })
})

describe("summarizeTunnelNames", () => {
  it("lists a few names and counts the rest", () => {
    const rules = ["a", "b", "c", "d", "e"].map((name) => ({ name }))
    expect(summarizeTunnelNames(rules.slice(0, 2))).toBe("a, b")
    expect(summarizeTunnelNames(rules)).toBe("a, b, c (+2)")
  })
})

describe("describeTransition", () => {
  it("reports drops, recoveries and failures only", () => {
    expect(describeTransition("running", "reconnecting")).toBe("lost")
    expect(describeTransition("reconnecting", "running")).toBe("restored")
    expect(describeTransition("reconnecting", "error")).toBe("failed")
    expect(describeTransition(undefined, "error")).toBe("failed")
    expect(describeTransition("error", "error")).toBeNull()
    expect(describeTransition("starting", "running")).toBeNull()
    expect(describeTransition("running", "stopped")).toBeNull()
  })
})
