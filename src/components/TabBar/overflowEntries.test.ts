import { describe, expect, it } from "vitest"

import { getFirstOverflowTarget, getOverflowEntries } from "@/components/TabBar/overflowEntries"
import type { Tab } from "@/types/tab"

const tab = (id: string, title: string, host?: string): Tab => ({
  id,
  title,
  type: host ? "ssh" : "terminal",
  isActive: false,
  connection: host ? { type: "ssh", host } : undefined,
})

const web = tab("web", "web", "web.example.com")
const db = tab("db", "db", "db.example.com")
const logs = tab("logs", "logs")
const panes = new Map([["web", { tabs: [web, db], isZoomed: false }]])

describe("getOverflowEntries", () => {
  it("lists every tab, and the panes of split ones, without a query", () => {
    const entries = getOverflowEntries([web, logs], panes, "")
    expect(
      entries.map((entry) => [entry.tab.id, entry.number, entry.panes.map(({ id }) => id)])
    ).toEqual([
      ["web", 1, ["web", "db"]],
      ["logs", 2, []],
    ])
  })

  it("finds a split tab by a pane out of sight, listing only the panes that match", () => {
    const entries = getOverflowEntries([web, logs], panes, "db.example")
    expect(entries).toHaveLength(1)
    expect(entries[0]).toMatchObject({ matched: false })
    expect(entries[0].panes.map(({ id }) => id)).toEqual(["db"])
    expect(getFirstOverflowTarget(entries)).toBe("db")
  })

  it("matches a tab by its number and opens the tab itself", () => {
    const entries = getOverflowEntries([web, logs], panes, "2")
    expect(entries.map((entry) => entry.tab.id)).toEqual(["logs"])
    expect(getFirstOverflowTarget(entries)).toBe("logs")
  })

  it("lists nothing when nothing matches", () => {
    expect(getOverflowEntries([web, logs], panes, "nope")).toEqual([])
    expect(getFirstOverflowTarget([])).toBeUndefined()
  })
})
