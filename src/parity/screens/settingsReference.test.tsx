// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { settingsReferenceFixture } from "@/parity/screens/settingsReferenceFixture"
import { SettingsReference } from "@/parity/screens/SettingsReference"

const mocks = vi.hoisted(() => ({ calls: [] as string[] }))

// The settings module imports the drag-and-drop settings page, whose library
// asks for a ResizeObserver before any test renders.
vi.hoisted(() => {
  if (typeof globalThis.ResizeObserver === "undefined") {
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver
  }
})

vi.mock("@tauri-apps/api/core", async () => {
  const { settingsReferenceFixture: fixture } =
    await import("@/parity/screens/settingsReferenceFixture")
  return {
    isTauri: () => false,
    invoke: vi.fn(async (command: string) => {
      mocks.calls.push(command)
      if (!Object.prototype.hasOwnProperty.call(fixture.commands, command)) {
        throw new Error(`unexpected command in the settings reference: ${command}`)
      }
      return structuredClone(fixture.commands[command as keyof typeof fixture.commands])
    }),
  }
})

function renderSettings() {
  return render(<SettingsReference onReady={() => {}} />)
}

beforeEach(() => {
  mocks.calls.length = 0
  localStorage.clear()
})

afterEach(cleanup)

describe("settings reference fixture", () => {
  it("answers the commands the settings page calls at mount", () => {
    expect(Object.keys(settingsReferenceFixture.commands).sort()).toEqual([
      "folder_menu_status",
      "get_secret_backend_status",
      "list_fonts",
      "load_app_state",
      "load_config",
      "load_custom_themes",
      "save_window_background",
      "shell_command_status",
    ])
    expect(settingsReferenceFixture.commands.load_config.language).toBe("en")
    expect(settingsReferenceFixture.commands.load_config.theme).toBe("default")
  })
})

describe("settings page reference", () => {
  it("opens on the production default General page", async () => {
    renderSettings()

    await waitFor(() => {
      expect(screen.getByRole("tab", { name: "General" }).getAttribute("aria-selected")).toBe(
        "true"
      )
    })
    expect(screen.getByRole("tabpanel").id).toBe("settings-panel-general")
    expect(document.querySelector('[data-parity="settings.layout.panel"]')).not.toBeNull()
  })

  it("opens the Font Family setting from the real search for font", async () => {
    renderSettings()
    await waitFor(() => {
      expect(screen.getByRole("tab", { name: "General" }).getAttribute("aria-selected")).toBe(
        "true"
      )
    })

    fireEvent.change(screen.getByRole("combobox", { name: "Search settings" }), {
      target: { value: "font" },
    })

    const results = await screen.findByRole("listbox", { name: "Search settings" })
    const options = within(results).getAllByRole("option")
    expect(options.map((option) => option.textContent)).toEqual([
      "TerminalFont Family",
      "TerminalFont Size",
      "AppearanceInterface font scaleChoose a text scale from 80% to 200%.",
    ])
    expect(options[0].getAttribute("data-parity")).toBe("settings.search.result.0")

    fireEvent.click(within(options[0]).getByText("Font Family"))

    await waitFor(() => {
      expect(screen.getByRole("tab", { name: "Terminal" }).getAttribute("aria-selected")).toBe(
        "true"
      )
    })
    expect(screen.getByRole("tab", { name: "General" }).getAttribute("aria-selected")).toBe("false")
    const panel = document.getElementById("settings-panel-terminal")
    expect(panel).not.toBeNull()
    expect(within(panel as HTMLElement).getByText("Font Family")).toBeTruthy()
    expect(screen.queryByRole("listbox", { name: "Search settings" })).toBeNull()
    expect(mocks.calls).toContain("list_fonts")
  })
})
