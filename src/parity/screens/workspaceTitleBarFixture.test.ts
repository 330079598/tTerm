import { describe, expect, it } from "vitest"

import { getPaneIds } from "@/lib/tabGroups"
import {
  workspaceTitleBarActiveTabId,
  workspaceTitleBarConfig,
  workspaceTitleBarFixture,
  workspaceTitleBarGroups,
  workspaceTitleBarTabs,
} from "@/parity/screens/workspaceTitleBarFixture"

describe("Windows workspace title bar fixture", () => {
  it("has two synthetic local tabs and one active tab", () => {
    expect(workspaceTitleBarTabs).toHaveLength(2)
    expect(new Set(workspaceTitleBarTabs.map((tab) => tab.id)).size).toBe(2)
    expect(workspaceTitleBarTabs.filter((tab) => tab.isActive).map((tab) => tab.id)).toEqual([
      workspaceTitleBarActiveTabId,
    ])
    expect(workspaceTitleBarTabs).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: "terminal",
          shellName: "pwsh",
          connection: expect.objectContaining({ cwd: "C:\\Users\\demo\\source\\app" }),
        }),
        expect.objectContaining({
          type: "terminal",
          shellName: "cmd",
          connection: expect.objectContaining({ cwd: "C:\\Users\\demo" }),
        }),
      ])
    )
  })

  it("keeps synthetic tabs free of connection and profile identities", () => {
    for (const tab of workspaceTitleBarTabs) {
      expect(tab.type).toBe("terminal")
      expect(tab.connection?.host).toBeUndefined()
      expect(tab.connection?.username).toBeUndefined()
      expect(tab.connection?.profileId).toBeUndefined()
      expect(tab.connection?.profileName).toBeUndefined()
    }
  })

  it("gives each tab one group with that tab's pane in front", () => {
    expect(workspaceTitleBarGroups).toHaveLength(workspaceTitleBarTabs.length)
    for (const [index, group] of workspaceTitleBarGroups.entries()) {
      expect(group.activeTabId).toBe(workspaceTitleBarTabs[index]?.id)
      expect(getPaneIds(group.root)).toEqual([group.activeTabId])
    }
  })

  it("normalizes the config and exposes every command the screen reads", () => {
    expect(workspaceTitleBarConfig.ui_style).toBe("classic")
    expect(workspaceTitleBarConfig.language).toBe("en")
    expect(workspaceTitleBarConfig.tab_icons.enabled).toBe(true)
    expect(Object.keys(workspaceTitleBarFixture.commands).sort()).toEqual([
      "get_secret_backend_status",
      "load_app_state",
      "load_config",
    ])
  })
})
