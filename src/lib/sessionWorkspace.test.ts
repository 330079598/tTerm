/// <reference types="node" />

import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { describe, expect, it } from "vitest"
import { getPaneIds, type TabGroup } from "@/lib/tabGroups"
import { restoreTabGroups, serializeTabGroups } from "@/lib/tabGroupsPersistence"
import { isPageTab, type Tab } from "@/types/tab"

interface SavedSession {
  tabs: Tab[]
  active_tab_id: string | null
  layout: unknown
  schema_version: number
  last_saved: number
}

function readFixture(name: string): SavedSession {
  const path = fileURLToPath(
    new URL(`../../crates/tterm-core/tests/fixtures/session/${name}`, import.meta.url)
  )
  return JSON.parse(readFileSync(path, "utf8")) as SavedSession
}

function persistable(tabs: Tab[]) {
  return tabs.filter((tab) => !isPageTab(tab) && tab.type !== "remote-file-editor")
}

function withGroupIds(groups: Omit<TabGroup, "id">[]): TabGroup[] {
  return groups.map((group, index) => ({ ...group, id: `group-${index + 1}` }))
}

describe("saved session workspace fixtures", () => {
  it("writes the same layout that the Rust decoder reads", () => {
    const saved = readFixture("tauri-v1-workspace.json")
    const tabs = persistable(saved.tabs)
    const tabIds = tabs.map((tab) => tab.id)

    expect(tabIds).toEqual(["local shell 1", "ssh:prod host"])
    expect(saved.active_tab_id).toBe("ssh:prod host")
    expect(saved.schema_version).toBe(1)

    const restored = restoreTabGroups(saved.layout, tabIds)
    expect(restored.map((group) => getPaneIds(group.root))).toEqual([
      ["local shell 1", "ssh:prod host"],
    ])
    expect(restored[0]).toMatchObject({
      activeTabId: "ssh:prod host",
      zoomedTabId: "ssh:prod host",
    })
    expect(serializeTabGroups(withGroupIds(restored), new Set(tabIds))).toEqual(saved.layout)
  })

  it("reads the native writer's unknown fields, resume and non-numeric ids", () => {
    const saved = readFixture("gpui-v1-workspace.json")
    const tabs = persistable(saved.tabs)
    const tabIds = tabs.map((tab) => tab.id)
    const ssh = tabs.find((tab) => tab.id === "ssh:prod host")

    expect(tabIds).toEqual(["local shell 1", "ssh:prod host"])
    expect(saved.tabs.some((tab) => tab.type === "settings")).toBe(false)
    expect(saved.tabs.some((tab) => tab.type === "remote-file-editor")).toBe(false)
    expect(ssh?.connection?.agentResume).toEqual({
      agent: "codex",
      sessionId: "session-alpha-42",
      cwd: "/srv/app",
    })
    expect(ssh?.connection?.jumpHosts?.[0]?.host).toBe("bastion.example")
    expect((ssh?.connection as Record<string, unknown> | undefined)?.futureConnectionField).toBe(
      "kept"
    )

    const encoded = JSON.stringify(saved)
    expect(encoded).not.toContain("must-not-survive")
    expect(encoded).not.toContain('"password":')
    expect(encoded).not.toContain('"privateKeyPassphrase":')

    const restored = restoreTabGroups(saved.layout, tabIds)
    expect(restored.map((group) => getPaneIds(group.root))).toEqual([
      ["local shell 1", "ssh:prod host"],
    ])
    expect(serializeTabGroups(withGroupIds(restored), new Set(tabIds))).toEqual(saved.layout)
  })
})
