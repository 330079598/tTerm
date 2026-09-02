// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { SettingsSidebar } from "@/components/SettingsDialog/SettingsSidebar"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => {
      const labels: Record<string, string> = {
        "settings.title": "Settings",
        "settings.profileGroups": "Groups",
        "settings.connection": "Connection",
        "settings.appearance": "Appearance",
        "settings.terminal": "Terminal",
        "terminalLogging.title": "Logs",
        "dataMigration.sidebar": "Backup & migration",
        "settings.security": "Security",
        "settings.general": "General",
        "settings.updates": "Updates",
      }
      return labels[key] ?? options?.defaultValue ?? key
    },
  }),
}))

afterEach(cleanup)

describe("SettingsSidebar", () => {
  it("exposes a single selected tab and moves focus with arrow keys", () => {
    const onTabChange = vi.fn()
    render(<SettingsSidebar activeTab="general" onTabChange={onTabChange} />)

    const general = screen.getByRole("tab", { name: "General" })
    const updates = screen.getByRole("tab", { name: "Updates" })

    expect(screen.getAllByRole("tab")).toHaveLength(9)
    expect(general.getAttribute("aria-selected")).toBe("true")
    expect(updates.getAttribute("aria-selected")).toBe("false")

    general.focus()
    fireEvent.keyDown(general, { key: "ArrowDown" })

    expect(onTabChange).toHaveBeenCalledWith("updates")
    expect(document.activeElement).toBe(updates)
  })

  it("supports Home and End navigation", () => {
    const onTabChange = vi.fn()
    render(<SettingsSidebar activeTab="security" onTabChange={onTabChange} />)

    const security = screen.getByRole("tab", { name: "Security" })
    fireEvent.keyDown(security, { key: "Home" })
    expect(onTabChange).toHaveBeenLastCalledWith("profile-groups")
    expect(document.activeElement).toBe(screen.getByRole("tab", { name: "Groups" }))

    fireEvent.keyDown(screen.getByRole("tab", { name: "Groups" }), { key: "End" })
    expect(onTabChange).toHaveBeenLastCalledWith("updates")
    expect(document.activeElement).toBe(screen.getByRole("tab", { name: "Updates" }))
  })
})
