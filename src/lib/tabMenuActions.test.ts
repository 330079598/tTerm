import { describe, expect, it } from "vitest"

import {
  getTabPinMenuAction,
  getTabRenameMenuAction,
  getTabSplitMenuActions,
  getWorkspaceTabMenuActions,
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

describe("getWorkspaceTabMenuActions", () => {
  const actions = (tab: Parameters<typeof getWorkspaceTabMenuActions>[0], ids = [tab.id]) =>
    getWorkspaceTabMenuActions(tab, ids, t).map((item) => item.action)

  it("offers a page only a new tab and closing", () => {
    const page = actions({ id: "a", title: "Settings", type: "settings", isActive: true })
    expect(page.slice(0, 3)).toEqual(["new", "", "close"])
    expect(page).not.toContain("rename")
  })

  it("offers a terminal its copies, splits, renaming and logging", () => {
    expect(actions({ id: "a", title: "tTerm", type: "terminal", isActive: true })).toEqual(
      expect.arrayContaining(["duplicate", "split-right", "rename", "close"])
    )
  })

  it("shows the duplicate shortcut beside the copy item", () => {
    const tab = { id: "a", title: "tTerm", type: "terminal", isActive: true } as const
    const duplicate = getWorkspaceTabMenuActions(tab, [tab.id], t).find(
      (item) => item.action === "duplicate"
    )
    expect(duplicate?.shortcut).toBe("tabs.duplicateActive")
  })

  it("edits the profile an SSH or serial tab was opened from", () => {
    const base = { id: "a", title: "tab", isActive: true }
    const profile = { profileId: "p" }
    expect(actions({ ...base, type: "ssh", connection: profile })).toContain("edit-connection")
    expect(actions({ ...base, type: "serial", connection: profile })).toContain("edit-connection")
    expect(actions({ ...base, type: "serial" })).not.toContain("edit-connection")
    expect(actions({ ...base, type: "terminal" })).not.toContain("edit-connection")
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
