import { describe, expect, it } from "vitest"
import {
  applyProfileToTab,
  buildConnectionFromProfile,
  profileTabIcon,
  sameConnectionTabs,
} from "@/lib/profileConnections"
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
  it("connects with a key's certificate, and drops it with the key", () => {
    const keyed = profile({
      auth_method: "key",
      private_key_path: "/keys/id",
      certificate_path: "/keys/id-ca-cert.pub",
    })
    expect(applyProfileToTab(tab(), keyed).connection?.certificatePath).toBe("/keys/id-ca-cert.pub")
    const keyedTab = applyProfileToTab(tab(), keyed)
    const agent = profile({ auth_method: "agent", certificate_path: "/keys/id-ca-cert.pub" })
    expect(applyProfileToTab(keyedTab, agent).connection?.certificatePath).toBeUndefined()
  })

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

describe("profileTabIcon", () => {
  it("gives a saved SSH connection's tabs its icon and its host's distribution", () => {
    const saved = profile({ tab_icon: { icon: "database" }, distro: "debian" })
    expect(buildConnectionFromProfile(saved)).toMatchObject({
      iconStyle: { icon: "database" },
      distroId: "debian",
    })
    expect(applyProfileToTab(tab({ distroId: "arch" }), saved).distroId).toBe("debian")
  })

  it("drops a distribution without an icon", () => {
    expect(profileTabIcon(profile({ distro: "beos" })).distroId).toBeUndefined()
  })
})

describe("sameConnectionTabs", () => {
  it("finds every open SSH tab of the same saved connection", () => {
    const first = tab()
    const second = tab({ id: "t2" })
    const other = tab({ id: "t3", connection: { type: "ssh", profileId: "p2" } })
    expect(sameConnectionTabs([first, second, other], first)).toEqual([first, second])
  })

  it("keeps a connection that was never saved to its own tab", () => {
    const quick = tab({ connection: { type: "ssh", host: "10.0.0.1" } })
    expect(sameConnectionTabs([quick, tab({ id: "t2" })], quick)).toEqual([quick])
  })
})

describe("serial profiles", () => {
  const board = profile({
    connection_type: "serial",
    name: "ESP32",
    host: undefined,
    serial: {
      port: "COM3",
      baudRate: 921600,
      dataBits: 8,
      parity: "none",
      stopBits: 1,
      flowControl: "none",
      lineEnding: "crlf",
      localEcho: false,
      timestamps: true,
    },
  })

  it("open a serial tab on the saved port", () => {
    const opened = buildConnectionFromProfile(board)
    expect(opened.type).toBe("serial")
    expect(opened.connection).toMatchObject({
      type: "serial",
      profileId: "p1",
      profileName: "ESP32",
      serial: { port: "COM3", baudRate: 921600, lineEnding: "crlf", timestamps: true },
    })
    expect(opened.connection?.host).toBeUndefined()
  })

  it("carry no SSH tab icon", () => {
    expect(profileTabIcon(board)).toEqual({})
  })
})
