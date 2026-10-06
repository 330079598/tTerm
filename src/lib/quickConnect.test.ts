import { describe, expect, it } from "vitest"

import {
  addRecentQuickConnection,
  applyQuickConnectAuth,
  buildQuickConnectTab,
  draftProfileFromInput,
  draftProfileFromTab,
  formatEndpoint,
  loadRecentQuickConnections,
  parseQuickConnectInput,
  QUICK_CONNECT_RECENTS_LIMIT,
  QUICK_CONNECT_RECENTS_STORAGE_KEY,
  resolveQuickConnectTarget,
  type SshConfigHost,
} from "@/lib/quickConnect"
import type { Tab } from "@/types/tab"

function configHost(overrides: Partial<SshConfigHost> = {}): SshConfigHost {
  return {
    hostPattern: "bastion",
    name: "bastion",
    host: "203.0.113.10",
    port: 2222,
    username: "ops",
    privateKeyPath: "~/.ssh/bastion",
    agentForward: false,
    keepaliveIntervalSecs: 15,
    keepaliveCountMax: 3,
    jumpHosts: [],
    skipped: false,
    ...overrides,
  }
}

function tabFor(connection: Parameters<typeof buildQuickConnectTab>[0]): Tab {
  return { id: "tab-1", isActive: true, ...buildQuickConnectTab(connection) }
}

describe("parseQuickConnectInput", () => {
  it.each([
    ["root@10.0.0.5", { username: "root", host: "10.0.0.5", port: undefined }],
    ["root@10.0.0.5:2222", { username: "root", host: "10.0.0.5", port: 2222 }],
    ["web-01.example.com", { username: undefined, host: "web-01.example.com", port: undefined }],
    ["admin@[2001:db8::1]:2200", { username: "admin", host: "2001:db8::1", port: 2200 }],
    ["fe80::1", { username: undefined, host: "fe80::1", port: undefined }],
    ["ssh -p 2222 deploy@web", { username: "deploy", host: "web", port: 2222 }],
    ["ssh -l deploy -p2222 web", { username: "deploy", host: "web", port: 2222 }],
  ])("reads %s", (input, expected) => {
    expect(parseQuickConnectInput(input)).toMatchObject(expected)
  })

  it("reads a jump host and a key file", () => {
    expect(parseQuickConnectInput("ssh -J jump@bastion:2200 -i ~/.ssh/k root@db")).toEqual({
      username: "root",
      host: "db",
      port: undefined,
      privateKeyPath: "~/.ssh/k",
      jumpHost: { username: "jump", host: "bastion", port: 2200 },
    })
  })

  it.each([
    "",
    "prod db",
    "ssh",
    "ssh -v root@host",
    "ssh root@host uptime",
    "root@host:70000",
    "root@host:abc",
    "@host",
    "ssh -J a,b root@host",
    "a/b",
  ])("does not read %j as a host", (input) => {
    expect(parseQuickConnectInput(input)).toBeNull()
  })
})

describe("resolveQuickConnectTarget", () => {
  it("needs a user name", () => {
    expect(resolveQuickConnectTarget({ host: "10.0.0.5" }, [])).toBeNull()
  })

  it("fills an alias in from ~/.ssh/config, letting what was typed win", () => {
    const hosts = [configHost()]
    expect(resolveQuickConnectTarget({ host: "bastion" }, hosts)).toMatchObject({
      title: "ops@bastion",
      username: "ops",
      host: "203.0.113.10",
      port: 2222,
      privateKeyPath: "~/.ssh/bastion",
    })
    expect(
      resolveQuickConnectTarget({ host: "bastion", username: "root", port: 22 }, hosts)
    ).toMatchObject({ title: "root@bastion", username: "root", port: 22 })
  })

  it("goes through a typed jump host as the same user unless one is given", () => {
    const resolved = resolveQuickConnectTarget(
      { username: "root", host: "db", jumpHost: { host: "bastion" } },
      []
    )
    expect(resolved?.jumpHosts).toEqual([
      { host: "bastion", port: 22, username: "root", authMethod: "auto" },
    ])
  })
})

describe("formatEndpoint", () => {
  it("leaves out the default port and brackets IPv6 before a port", () => {
    expect(formatEndpoint({ username: "u", host: "h", port: 22 })).toBe("u@h")
    expect(formatEndpoint({ host: "2001:db8::1", port: 2200 })).toBe("[2001:db8::1]:2200")
  })
})

describe("applyQuickConnectAuth", () => {
  const connection = { title: "root@db", username: "root", host: "db", port: 22 }

  it("switches the tab to a method that needs nothing typed", () => {
    const tab = applyQuickConnectAuth(tabFor(connection), {
      method: "key",
      privateKeyPath: "/home/me/.ssh/id_ed25519",
      reusable: true,
    })
    expect(tab.connection).toMatchObject({
      authMethod: "key",
      privateKeyPath: "/home/me/.ssh/id_ed25519",
    })
  })

  it("keeps asking when the user had to type something", () => {
    const tab = applyQuickConnectAuth(tabFor(connection), { method: "password", reusable: false })
    expect(tab.connection?.authMethod).toBe("auto")
    expect(tab.quickConnectAuth?.method).toBe("password")
  })

  it("leaves tabs with a configured method alone", () => {
    const tab = { ...tabFor(connection), connection: { authMethod: "agent" as const } }
    expect(applyQuickConnectAuth(tab, { method: "password", reusable: false })).toBe(tab)
  })
})

describe("draft profiles", () => {
  it("saves the method that worked", () => {
    const tab = applyQuickConnectAuth(
      tabFor({ title: "root@db", username: "root", host: "db", port: 2222 }),
      { method: "interactive", reusable: false }
    )
    expect(draftProfileFromTab(tab)).toMatchObject({
      name: "root@db",
      connection_type: "ssh",
      host: "db",
      port: 2222,
      username: "root",
      auth_method: "interactive",
    })
  })

  it("prefills the connection dialog from what was typed", () => {
    expect(draftProfileFromInput("10.0.0.5:2200", [])).toMatchObject({
      host: "10.0.0.5",
      port: 2200,
      username: "",
      auth_method: "password",
    })
    expect(draftProfileFromInput("prod db", [])).toBeNull()
  })
})

describe("recent quick connections", () => {
  const connection = { title: "root@db", username: "root", host: "db", port: 22 }

  it("moves a repeated connection to the front and keeps a bounded list", () => {
    let recents = addRecentQuickConnection([], connection, 1)
    for (let index = 0; index < QUICK_CONNECT_RECENTS_LIMIT; index += 1) {
      recents = addRecentQuickConnection(
        recents,
        { ...connection, host: `h${index}`, title: `h${index}` },
        index + 2
      )
    }
    recents = addRecentQuickConnection(recents, connection, 100)
    expect(recents).toHaveLength(QUICK_CONNECT_RECENTS_LIMIT)
    expect(recents[0]).toMatchObject({ host: "db", lastUsedAt: 100 })
    expect(recents.filter((recent) => recent.host === "db")).toHaveLength(1)
  })

  it("ignores stored entries it cannot read", () => {
    const storage = {
      getItem: (key: string) =>
        key === QUICK_CONNECT_RECENTS_STORAGE_KEY
          ? JSON.stringify([{ ...connection, lastUsedAt: 1 }, { host: "broken" }])
          : null,
    }
    expect(loadRecentQuickConnections(storage)).toHaveLength(1)
    expect(loadRecentQuickConnections({ getItem: () => "{not json" })).toEqual([])
  })
})
