import { describe, expect, it } from "vitest"

import {
  getTabConnectionMenuAction,
  getTabPinMenuAction,
  getTabRenameMenuAction,
  getTabSplitMenuActions,
  getPaneMenuActions,
  tabConnectionActionLabel,
} from "@/lib/tabMenuActions"

const t = ((key: string) => key) as Parameters<typeof getTabSplitMenuActions>[1]

describe("getTabSplitMenuActions", () => {
  it("offers the four splits for terminal tabs only", () => {
    expect(getTabSplitMenuActions({ type: "ssh" }, t).map((item) => item.action)).toEqual([
      "split-right",
      "split-left",
      "split-down",
      "split-above",
    ])
    expect(getTabSplitMenuActions({ type: "terminal" }, t)).toHaveLength(4)
    expect(getTabSplitMenuActions({ type: "settings" }, t)).toEqual([])
  })
})

describe("getTabPinMenuAction", () => {
  it("pins or unpins an SSH tab's connection header", () => {
    const actions = (tab: Parameters<typeof getTabPinMenuAction>[0]) =>
      getTabPinMenuAction(tab, t).map((item) => item.action)
    expect(actions({ type: "ssh", connectionHeaderPinned: false })).toEqual(["pin-header"])
    expect(actions({ type: "ssh" })).toEqual(["unpin-header"])
    expect(actions({ type: "terminal" })).toEqual([])
  })
})

describe("getPaneMenuActions", () => {
  const actions = (tab: Parameters<typeof getPaneMenuActions>[0], isZoomed = false) =>
    getPaneMenuActions(tab, { isZoomed }, t).map((item) => item.action)

  it("offers a page only the pane's layout and closing", () => {
    const page = actions({ id: "a", title: "Settings", type: "settings", isActive: true })
    expect(page).toEqual([
      "toggle-zoom",
      "move-to-new-tab",
      "ungroup",
      "layout-templates",
      "",
      "close",
    ])
  })

  it("offers a terminal its copies, splits, renaming and the pane's layout", () => {
    expect(actions({ id: "a", title: "tTerm", type: "terminal", isActive: true })).toEqual(
      expect.arrayContaining([
        "duplicate",
        "split-right",
        "rename",
        "toggle-zoom",
        "move-to-new-tab",
        "ungroup",
        "close",
      ])
    )
  })

  it("restores a zoomed pane instead of zooming it", () => {
    const tab = { id: "a", title: "tTerm", type: "terminal", isActive: true } as const
    const zoom = (isZoomed: boolean) =>
      getPaneMenuActions(tab, { isZoomed }, t).find((item) => item.action === "toggle-zoom")
    expect(zoom(false)?.label).toBe("workspace.zoomPane")
    expect(zoom(true)?.label).toBe("workspace.unzoomPane")
  })

  it("shows the duplicate shortcut beside the copy item", () => {
    const tab = { id: "a", title: "tTerm", type: "terminal", isActive: true } as const
    const duplicate = getPaneMenuActions(tab, { isZoomed: false }, t).find(
      (item) => item.action === "duplicate"
    )
    expect(duplicate?.shortcut).toBe("tabs.duplicateActive")
  })

  it("edits the profile a session tab was opened from", () => {
    const base = { id: "a", title: "tab", isActive: true }
    const profile = { profileId: "p" }
    expect(actions({ ...base, type: "ssh", connection: profile })).toContain("edit-connection")
    expect(actions({ ...base, type: "serial", connection: profile })).toContain("edit-connection")
    expect(actions({ ...base, type: "terminal", connection: profile })).toContain("edit-connection")
    expect(actions({ ...base, type: "serial" })).not.toContain("edit-connection")
  })
})

describe("getTabConnectionMenuAction", () => {
  const labels = (tab: Parameters<typeof getTabConnectionMenuAction>[0]) =>
    getTabConnectionMenuAction(tab, t).map((item) => item.label)

  it("saves an SSH or local tab that has no profile, and edits one that has", () => {
    expect(labels({ type: "ssh" })).toEqual(["contextMenu.saveConnection"])
    expect(labels({ type: "terminal" })).toEqual(["contextMenu.saveLocalConnection"])
    expect(labels({ type: "terminal", connection: { profileId: "p" } })).toEqual([
      "contextMenu.editConnection",
    ])
  })

  it("shows the shortcut, which acts only where the item is offered", () => {
    expect(getTabConnectionMenuAction({ type: "terminal" }, t)[0].shortcut).toBe(
      "tabs.saveConnection"
    )
    expect(tabConnectionActionLabel({ type: "terminal" })).toBe("contextMenu.saveLocalConnection")
    expect(tabConnectionActionLabel({ type: "serial" })).toBeNull()
    expect(tabConnectionActionLabel({ type: "settings" })).toBeNull()
  })

  it("offers nothing for a serial tab without a profile or a remote file", () => {
    expect(labels({ type: "serial" })).toEqual([])
    expect(labels({ type: "remote-file-editor", connection: { profileId: "p" } })).toEqual([])
  })
})

describe("getTabRenameMenuAction", () => {
  it("edits a terminal tab, whose icon can be chosen, and renames others", () => {
    expect(getTabRenameMenuAction({ type: "terminal" }, t).label).toBe("contextMenu.editTab")
    expect(getTabRenameMenuAction({ type: "ssh" }, t).label).toBe("contextMenu.editTab")
    expect(getTabRenameMenuAction({ type: "remote-file-editor" }, t).label).toBe(
      "contextMenu.renameTab"
    )
  })
})
