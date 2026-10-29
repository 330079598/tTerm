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
  group_path: "Production",
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
    profileRecents: [],
    shells: [],
    shellConfig: {
      terminal_shell: "auto",
      terminal_shell_custom_path: "",
      terminal_shell_custom_args: "",
      terminal_shell_id: "",
      local_shells: [],
      hidden_shells: [],
    },
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

  it("lists the saved connections used lately among the recents, newest first", () => {
    expect(
      keys({
        profiles: [profile(), profile({ id: "p2", name: "db-01" })],
        recents: [recent({ lastUsedAt: 2000 })],
        profileRecents: [
          { profileId: "p2", lastUsedAt: 3000 },
          { profileId: "p1", lastUsedAt: 1000 },
        ],
      }).filter((key) => key.startsWith("recent"))
    ).toEqual(["recent-profile:p2", "recent:admin@db.example.com:22:2000", "recent-profile:p1"])
  })

  it("leaves out a recent whose saved connection was deleted", () => {
    expect(keys({ profileRecents: [{ profileId: "gone", lastUsedAt: 1 }] })).toEqual([
      "terminal",
      "new-connection",
    ])
  })

  it("lets a saved recent be forgotten by its profile", () => {
    const item = build({
      profiles: [profile()],
      profileRecents: [{ profileId: "p1", lastUsedAt: 1 }],
    }).find((candidate) => candidate.key === "recent-profile:p1")
    expect(item?.recent).toEqual({ kind: "profile", profileId: "p1" })
  })

  it("puts the matching saved connections used lately first", () => {
    const items = build({
      input: "web",
      profiles: [
        profile(),
        profile({ id: "p2", name: "web-02" }),
        profile({ id: "p3", name: "web-03" }),
      ],
      profileRecents: [{ profileId: "p3", lastUsedAt: 1 }],
    })
    expect(items.filter((item) => item.section === "profiles").map((item) => item.key)).toEqual([
      "profile:p3",
      "profile:p1",
      "profile:p2",
    ])
  })

  it("puts a typed connection first", () => {
    expect(keys({ input: "admin@example.org" })[0]).toBe("typed")
  })

  it("offers the default shell first, then the others not hidden, the user's own last", () => {
    const items = build({
      shells: [
        { shell: "auto", label: "Auto", source: "tTerm default" },
        { shell: "cmd", label: "cmd", source: "C:\\Windows\\System32\\cmd.exe" },
        { shell: "pwsh", label: "pwsh", source: "C:\\PowerShell\\7\\pwsh.exe" },
        { shell: "wsl", label: "WSL", source: "C:\\Windows\\System32\\wsl.exe" },
      ],
      shellConfig: {
        terminal_shell: "pwsh",
        terminal_shell_custom_path: "",
        terminal_shell_custom_args: "",
        terminal_shell_id: "",
        local_shells: [{ id: "nu", name: "Nushell", path: "nu", args: "-l" }],
        hidden_shells: ["cmd"],
      },
    }).filter((item) => item.section === "localTerminal")
    expect(items.map((item) => [item.key, item.label])).toEqual([
      ["terminal", "connection.terminalShellOptions.pwsh"],
      ["terminal:wsl", "connection.terminalShellOptions.wsl"],
      ["terminal:custom:nu", "Nushell"],
    ])
    expect(items[2].action).toEqual({
      kind: "terminal",
      shell: { shell: "custom", customPath: "nu", customArgs: "-l", shellId: "nu" },
    })
  })
})
