import { describe, expect, it } from "vitest"

import {
  CLOSED_TABS_LIMIT,
  closedEntries,
  closedTabSnapshot,
  paneAnchor,
  placeReopenedTabs,
  pushClosedEntries,
} from "@/lib/closedTabs"
import { paneLeaf, removeTabsFromGroups, type PaneNode, type TabGroup } from "@/lib/tabGroups"
import type { Tab } from "@/types/tab"

const tab = (id: string, overrides: Partial<Tab> = {}): Tab => ({
  id,
  title: id,
  type: "terminal",
  isActive: false,
  sessionNonce: 3,
  pid: 42,
  connection: { type: "terminal", cwd: `/work/${id}` },
  ...overrides,
})

describe("closedTabSnapshot", () => {
  it("keeps a local tab's directory but not its session", () => {
    const snapshot = closedTabSnapshot(tab("a", { autoTitle: true }))
    expect(snapshot).toMatchObject({ title: "a", autoTitle: true, connection: { cwd: "/work/a" } })
    expect(snapshot).not.toHaveProperty("id")
    expect(snapshot).not.toHaveProperty("pid")
    expect(snapshot).not.toHaveProperty("sessionNonce")
  })

  it("forgets the passwords typed for a connection", () => {
    const snapshot = closedTabSnapshot(
      tab("b", {
        type: "ssh",
        quickConnect: true,
        connection: {
          type: "ssh",
          host: "web",
          password: "secret",
          privateKeyPassphrase: "phrase",
          jumpHosts: [
            { host: "bastion", port: 22, username: "me", authMethod: "password", password: "x" },
          ],
        },
      })
    )
    expect(snapshot?.quickConnect).toBe(false)
    expect(snapshot?.connection?.host).toBe("web")
    expect(snapshot?.connection?.password).toBeUndefined()
    expect(snapshot?.connection?.privateKeyPassphrase).toBeUndefined()
    expect(snapshot?.connection?.jumpHosts?.[0]).toMatchObject({ host: "bastion" })
    expect(snapshot?.connection?.jumpHosts?.[0].password).toBeUndefined()
  })

  it("keeps no pages or remote files", () => {
    expect(closedTabSnapshot(tab("s", { type: "settings" }))).toBeNull()
    expect(closedTabSnapshot(tab("f", { type: "remote-file-editor" }))).toBeNull()
  })
})

const split = (
  orientation: "horizontal" | "vertical",
  children: PaneNode[],
  sizes: number[]
): PaneNode => ({ type: "split", orientation, children, sizes })

// web | (api over logs), with logs a settings page.
const splitGroup: TabGroup = {
  id: "g1",
  root: split(
    "horizontal",
    [paneLeaf("web"), split("vertical", [paneLeaf("api"), paneLeaf("logs")], [0.5, 0.5])],
    [0.6, 0.4]
  ),
  activeTabId: "api",
}
const single: TabGroup = { id: "g2", root: paneLeaf("solo"), activeTabId: "solo" }
const allTabs = [tab("web"), tab("api"), tab("logs", { type: "settings" }), tab("solo")]

describe("closedEntries", () => {
  it("keeps a whole tab of the bar with its split, leaving out its pages", () => {
    const [entry] = closedEntries(allTabs, [single, splitGroup], ["web", "api", "logs"])
    expect(entry.tabs.map((closed) => closed.title)).toEqual(["web", "api"])
    expect(entry.layout).toEqual({
      kind: "group",
      shape: {
        type: "split",
        orientation: "horizontal",
        children: [{ type: "pane" }, { type: "pane" }],
        sizes: [0.6, 0.4],
      },
      activeIndex: 1,
      index: 1,
    })
  })

  it("keeps a pane closed out of a tab that stays open, with that tab's split", () => {
    const [entry] = closedEntries(allTabs, [splitGroup], ["api"])
    expect(entry.tabs.map((closed) => closed.title)).toEqual(["api"])
    expect(entry.layout).toMatchObject({
      kind: "pane",
      closedId: "api",
      previousRoot: splitGroup.root,
      anchor: { tabId: "logs", direction: "above" },
    })
  })

  it("makes one closing for each tab of the bar closed, in the bar's order", () => {
    const entries = closedEntries(allTabs, [single, splitGroup], ["solo", "web", "api", "logs"])
    expect(entries.map((entry) => entry.layout.kind)).toEqual(["group", "group"])
    expect(entries[0].tabs[0].title).toBe("solo")
  })
})

describe("paneAnchor", () => {
  it("goes back after the pane before it, else before the one after it", () => {
    expect(paneAnchor(splitGroup.root, "logs")).toEqual({ tabId: "api", direction: "below" })
    expect(paneAnchor(splitGroup.root, "web")).toEqual({ tabId: "api", direction: "left" })
    expect(paneAnchor(paneLeaf("solo"), "solo")).toBeNull()
  })
})

describe("placeReopenedTabs", () => {
  let count = 0
  const newId = () => `new-${++count}`

  it("puts a whole tab of the bar back in its place with its split", () => {
    const [entry] = closedEntries(allTabs, [single, splitGroup], ["web", "api", "logs"])
    const groups = placeReopenedTabs([single], entry.layout, ["w2", "a2"], newId)
    expect(groups.map((group) => group.id)).toEqual(["g2", "new-1"])
    expect(groups[1]).toMatchObject({
      activeTabId: "a2",
      root: split("horizontal", [paneLeaf("w2"), paneLeaf("a2")], [0.6, 0.4]),
    })
  })

  it("puts a pane back exactly where it was while its tab is as it was left", () => {
    const [entry] = closedEntries(allTabs, [splitGroup], ["api"])
    const after = removeTabsFromGroups([splitGroup], ["api"])
    const [group] = placeReopenedTabs(after, entry.layout, ["a2"], newId)
    expect(group.root).toEqual(
      split(
        "horizontal",
        [paneLeaf("web"), split("vertical", [paneLeaf("a2"), paneLeaf("logs")], [0.5, 0.5])],
        [0.6, 0.4]
      )
    )
    expect(group.activeTabId).toBe("a2")
  })

  it("puts a pane beside its neighbour once its tab has changed", () => {
    const [entry] = closedEntries(allTabs, [splitGroup], ["api"])
    const [after] = removeTabsFromGroups([splitGroup], ["api"])
    const resized = { ...after, root: { ...after.root, sizes: [0.3, 0.7] } as PaneNode }
    const [group] = placeReopenedTabs([resized], entry.layout, ["a2"], newId)
    expect(group.root).toEqual(
      split(
        "horizontal",
        [paneLeaf("web"), split("vertical", [paneLeaf("a2"), paneLeaf("logs")], [0.5, 0.5])],
        [0.3, 0.7]
      )
    )
  })

  it("leaves a pane whose neighbour is gone to a tab of its own", () => {
    const [entry] = closedEntries(allTabs, [splitGroup], ["api"])
    expect(placeReopenedTabs([single], entry.layout, ["a2"], newId)).toEqual([single])
  })
})

describe("pushClosedEntries", () => {
  it("adds the closings last and drops the oldest past the limit", () => {
    const entries = Array.from({ length: CLOSED_TABS_LIMIT }, (_, index) =>
      closedEntries(
        [tab(`t${index}`)],
        [{ id: "g", root: paneLeaf(`t${index}`), activeTabId: `t${index}` }],
        [`t${index}`]
      )
    ).flat()
    const latest = closedEntries(
      [tab("new")],
      [single, { id: "n", root: paneLeaf("new"), activeTabId: "new" }],
      ["new"]
    )
    const closed = pushClosedEntries(entries, latest)
    expect(closed).toHaveLength(CLOSED_TABS_LIMIT)
    expect(closed[closed.length - 1].tabs[0].title).toBe("new")
    expect(closed[0].tabs[0].title).toBe("t1")
  })
})
