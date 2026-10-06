import { describe, expect, it } from "vitest"
import { applyProfileToTab } from "@/lib/profileConnections"
import type { SavedProfile, Tab } from "@/types/tab"

const profile = (overrides: Partial<SavedProfile> = {}): SavedProfile => ({
  id: "p1",
  name: "web",
  group: "",
  connection_type: "ssh",
  host: "10.0.0.1",
  port: 22,
  username: "root",
  remember_password: true,
  auth_method: "password",
  keepalive_interval_secs: 15,
  keepalive_count_max: 3,
  ...overrides,
})

const tab = (overrides: Partial<Tab> = {}): Tab => ({
  id: "t1",
  title: "web",
  type: "ssh",
  isActive: true,
  sessionNonce: 2,
  connection: {
    type: "ssh",
    profileId: "p1",
    profileName: "web",
    host: "10.0.0.1",
    port: 22,
    username: "root",
    authMethod: "password",
    password: "typed",
  },
  ...overrides,
})

describe("applyProfileToTab", () => {
  it("connects with the saved settings and keeps the session", () => {
    const next = applyProfileToTab(tab(), profile({ port: 2222, encoding: "gbk" }))
    expect(next.sessionNonce).toBe(2)
    expect(next.connection).toMatchObject({ port: 2222, encoding: "gbk" })
  })

  it("keeps a typed password only for the same account", () => {
    expect(applyProfileToTab(tab(), profile({ login_script: "ls" })).connection?.password).toBe(
      "typed"
    )
    expect(
      applyProfileToTab(tab(), profile({ username: "deploy" })).connection?.password
    ).toBeUndefined()
  })

  it("follows a rename unless the tab was renamed", () => {
    expect(applyProfileToTab(tab(), profile({ name: "api" })).title).toBe("api")
    expect(applyProfileToTab(tab({ title: "mine" }), profile({ name: "api" })).title).toBe("mine")
    const copy = applyProfileToTab(
      tab({ title: "web-2", duplicateBaseTitle: "web" }),
      profile({ name: "api" })
    )
    expect(copy).toMatchObject({ title: "api-2", duplicateBaseTitle: "api" })
  })
})
