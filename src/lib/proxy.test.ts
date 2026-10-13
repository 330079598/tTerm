import { describe, expect, it } from "vitest"

import {
  countProfilesUsingProxy,
  normalizeProxyChoice,
  parseProxyChoiceKey,
  proxyAddress,
  proxyChoiceKey,
  proxyChoicePayload,
  validateProxyDraft,
} from "@/lib/proxy"

describe("proxy choices", () => {
  it("treat anything unknown as the default proxy", () => {
    expect(normalizeProxyChoice(undefined)).toEqual({ mode: "default" })
    expect(normalizeProxyChoice({ mode: "server" })).toEqual({ mode: "default" })
    expect(normalizeProxyChoice({ mode: "direct" })).toEqual({ mode: "direct" })
    expect(normalizeProxyChoice({ mode: "server", id: "p1" })).toEqual({
      mode: "server",
      id: "p1",
    })
  })

  it("round-trip through select values", () => {
    for (const choice of [
      { mode: "default" } as const,
      { mode: "direct" } as const,
      { mode: "server", id: "a:b" } as const,
    ]) {
      expect(parseProxyChoiceKey(proxyChoiceKey(choice))).toEqual(choice)
    }
    expect(parseProxyChoiceKey("server:")).toEqual({ mode: "default" })
  })

  it("leave the default out of what is stored", () => {
    expect(proxyChoicePayload({ mode: "default" })).toBeUndefined()
    expect(proxyChoicePayload({ mode: "direct" })).toEqual({ mode: "direct" })
  })

  it("count the profiles that chose a server", () => {
    const profiles = [
      { proxy: { mode: "server", id: "p1" } },
      { proxy: { mode: "server", id: "p2" } },
      { proxy: { mode: "direct" } },
      {},
    ]
    expect(countProfilesUsingProxy(profiles, "p1")).toBe(1)
    expect(countProfilesUsingProxy(profiles, "p3")).toBe(0)
  })
})

describe("proxy servers", () => {
  it("bracket IPv6 addresses", () => {
    expect(proxyAddress({ host: "127.0.0.1", port: 7890 })).toBe("127.0.0.1:7890")
    expect(proxyAddress({ host: "::1", port: 1080 })).toBe("[::1]:1080")
  })

  it("report the first field that cannot be saved", () => {
    const valid = { name: "Office", host: "proxy.corp", port: 8080 }
    expect(validateProxyDraft(valid)).toBeNull()
    expect(validateProxyDraft({ ...valid, name: " " })).toBe("name")
    expect(validateProxyDraft({ ...valid, host: "a b" })).toBe("host")
    expect(validateProxyDraft({ ...valid, port: 0 })).toBe("port")
    expect(validateProxyDraft({ ...valid, port: 70000 })).toBe("port")
  })
})
