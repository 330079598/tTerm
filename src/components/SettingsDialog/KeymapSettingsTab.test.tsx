// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { KeymapSettingsTab } from "@/components/SettingsDialog/KeymapSettingsTab"

const mocks = vi.hoisted(() => ({
  saveSettings: vi.fn().mockResolvedValue(true),
  setDispatchSuppressed: vi.fn(),
}))

vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => "macos" }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock("@/contexts/ConfigContext", () => ({
  useConfig: () => ({ config: { keymap: { bindings: {} } } }),
}))
vi.mock("@/contexts/KeymapContext", () => ({
  useKeymap: () => ({ bindings: {}, setDispatchSuppressed: mocks.setDispatchSuppressed }),
}))
vi.mock("@/hooks/useSettingsSave", () => ({
  useSettingsSave: () => ({ saveSettings: mocks.saveSettings }),
}))

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

function startRecordingFirstAction() {
  fireEvent.click(screen.getAllByRole("button", { name: "keymap.recordFor" })[0])
}

describe("KeymapSettingsTab recorder", () => {
  it("records a modified Backspace chord on macOS", async () => {
    render(<KeymapSettingsTab />)
    startRecordingFirstAction()

    fireEvent.keyDown(window, {
      key: "Backspace",
      code: "Backspace",
      metaKey: true,
    })

    await waitFor(() =>
      expect(mocks.saveSettings).toHaveBeenCalledWith({
        keymap: { bindings: { "workspace.newTab": ["mod+backspace"] } },
      })
    )
  })

  it("clears a binding for an unmodified Backspace", async () => {
    render(<KeymapSettingsTab />)
    startRecordingFirstAction()

    fireEvent.keyDown(window, { key: "Backspace", code: "Backspace" })

    await waitFor(() =>
      expect(mocks.saveSettings).toHaveBeenCalledWith({
        keymap: { bindings: { "workspace.newTab": null } },
      })
    )
  })
})
