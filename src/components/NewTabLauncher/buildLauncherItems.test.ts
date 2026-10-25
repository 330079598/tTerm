import type { TFunction } from "i18next"
import { describe, expect, it } from "vitest"

import {
  buildLauncherItems,
  type LauncherItemsInput,
} from "@/components/NewTabLauncher/buildLauncherItems"
import type { RecentQuickConnection } from "@/lib/quickConnect"
import type { SavedProfile } from "@/types/tab"

const t = ((key: string) => key) as TFunction

const profile = (overrides: Partial<SavedProfile> = {}): SavedProfile => ({
  id: "p1",
  name: "web-01",
  group: "Production",
  connection_type: "ssh",
  host: "10.0.0.5",
  port: 22,
  username: "root",
  remember_password: true,
  keepalive_interval_secs: 15,
  keepalive_count_max: 3,
  ...overrides,
})

const recent = (overrides: Partial<RecentQuickConnection> = {}): RecentQuickConnection => ({
  title: "admin@db.example.com",
  host: "db.example.com",
  port: 22,
  username: "admin",
  lastUsedAt: 1000,
  ...overrides,
})

const build = (overrides: Partial<LauncherItemsInput> = {}) =>
  buildLauncherItems({
    input: "",
    configHosts: [],
    profiles: [],
    recents: [],
    shells: [],
    terminalShell: "auto",
    terminalShellCustomPath: "",
    t,
    ...overrides,
  })

const keys = (overrides: Partial<LauncherItemsInput> = {}) =>
  build(overrides).map((item) => item.key)

describe("buildLauncherItems", () => {
  it("offers a terminal, a new connection and the recents when nothing is typed", () => {
    expect(keys({ profiles: [profile()], recents: [recent()] })).toEqual([
      "terminal",
      "new-connection",
      "recent:admin@db.example.com:22:1000",
    ])
  })

  it("lists the saved connections that match what was typed", () => {
    const items = build({
      input: "web root",
      profiles: [profile(), profile({ id: "p2", name: "db-01", username: "postgres" })],
    })
    expect(items.filter((item) => item.section === "profiles").map((item) => item.key)).toEqual([
      "profile:p1",
    ])
  })

  it("puts a typed connection first", () => {
    expect(keys({ input: "admin@example.org" })[0]).toBe("typed")
  })
})
